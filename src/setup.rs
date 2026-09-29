//! 首次配置辅助：解析浏览器里复制的 cURL / 请求头，生成 config.json。
//!
//! 设计目标：任何用户只要从自己浏览器的开发者工具里复制一条 U校园 请求
//! （右键 → Copy → Copy as cURL (bash)）粘贴进来，就能完成配置。

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

use crate::config::Config;

/// 前端 `common-*.js` 里 `_generateJwtToken()` 用的固定密钥（与页面行为一致）。
const ANNOTATOR_SECRET: &str = "a824b379f126b8b7aa5e33dee83fb0a05aa7462c";
const ANNOTATOR_ISS: &str = "c4f772063dcfa98e9c50";
const ANNOTATOR_AUD: &str = "edx.unipus.cn";

/// 从粘贴内容里提取出来的配置片段。
#[derive(Debug, Default, PartialEq)]
pub struct PastedRequest {
    pub url: Option<String>,
    pub cookie: Option<String>,
    pub authorization: Option<String>,
    pub annotator: Option<String>,
    pub u_school: Option<String>,
    pub open_id: Option<String>,
    pub class_id: Option<String>,
    pub curricula_id: Option<String>,
    pub course_id: Option<String>,
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix('\'')
        .and_then(|x| x.strip_suffix('\''))
        .unwrap_or(s);
    let s = s
        .strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .unwrap_or(s);
    s.trim().to_string()
}

/// 把 cURL 行、`-H` 头、纯 cookie 串等统一解析成字段。
pub fn parse_pasted(text: &str) -> PastedRequest {
    let mut out = PastedRequest::default();
    let mut headers: Vec<(String, String)> = Vec::new();

    // 有人会把 cURL 粘成一行（或去掉续行符），这里先把各个参数拆到独立行再解析。
    let normalized = split_flags(text);

    for line in normalized.lines() {
        let trimmed = line
            .trim()
            .trim_end_matches('\\')
            .trim()
            .trim_end_matches('^')
            .trim();
        if trimmed.is_empty() {
            continue;
        }
        // URL 出现的几种形式：curl 'URL' / curl --url 'URL' / GET URL HTTP/1.1
        if out.url.is_none() {
            let mut tokens = trimmed.split_whitespace();
            let first = tokens.next().unwrap_or("");
            let rest = match first {
                "curl" | "curl.exe" => {
                    let mut t = tokens.next();
                    if matches!(t, Some("--url")) {
                        t = tokens.next();
                    }
                    t.unwrap_or("")
                }
                "--url" => tokens.next().unwrap_or(""),
                "GET" | "POST" | "PUT" | "DELETE" | "OPTIONS" => tokens.next().unwrap_or(""),
                _ => "",
            };
            let v = unquote(rest);
            if v.starts_with("http") {
                out.url = Some(v);
            }
        }
        // -H 'name: value' / --header
        let is_header_flag = trimmed.starts_with("-H") || trimmed.starts_with("--header");
        if is_header_flag {
            let rest = trimmed
                .trim_start_matches("--header")
                .trim_start_matches("-H")
                .trim();
            let v = unquote(rest);
            push_header(&mut headers, &v);
            continue;
        }
        // -b 'cookie' / --cookie
        if trimmed.starts_with("-b ") || trimmed.starts_with("--cookie") {
            let rest = trimmed
                .trim_start_matches("--cookie")
                .trim_start_matches("-b")
                .trim();
            let v = unquote(rest);
            if !v.is_empty() {
                out.cookie = Some(v);
            }
            continue;
        }
        // 直接粘贴的请求头块：Name: value
        if let Some((k, v)) = trimmed.split_once(':') {
            let k = k.trim();
            if !k.is_empty()
                && k.len() < 40
                && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !k.eq_ignore_ascii_case("curl")
            {
                push_header(&mut headers, &format!("{}: {}", k, v.trim()));
            }
        }
    }

    for (k, v) in &headers {
        match k.as_str() {
            "cookie" => out.cookie = Some(v.clone()),
            "authorization" => {
                out.authorization = Some(v.trim_start_matches("Bearer ").trim().to_string())
            }
            "x-annotator-auth-token" => out.annotator = Some(v.clone()),
            "u-school" => out.u_school = Some(v.clone()),
            "u-openid" => out.open_id = Some(v.clone()),
            _ => {}
        }
    }

    // 只粘贴了 cookie 本身的情况（排除误把整条 curl 当 cookie）
    let plain_cookie = text.contains("jwt=")
        && !text.contains("curl")
        && !text.contains(" -H ")
        && !text.contains("http")
        && text.matches('=').count() >= 1;
    if out.cookie.is_none() && plain_cookie {
        out.cookie = Some(unquote(text.trim()));
    }

    if let Some(cookie) = out.cookie.clone() {
        if out.open_id.is_none() {
            out.open_id = cookie_open_id(&cookie);
        }
    }
    if let Some(url) = out.url.clone() {
        if out.course_id.is_none() {
            out.course_id = url_course_id(&url);
        }
        if out.open_id.is_none() {
            out.open_id = url_open_id(&url);
        }
        if out.class_id.is_none() {
            out.class_id = query_param(&url, "cid");
        }
        if out.curricula_id.is_none() {
            out.curricula_id = query_param(&url, "cloudCurriculaId");
        }
    }
    // referer 里也可能带 cid / cloudCurriculaId
    for (k, v) in &headers {
        if k == "referer" {
            if out.class_id.is_none() {
                out.class_id = query_param(v, "cid");
            }
            if out.curricula_id.is_none() {
                out.curricula_id = query_param(v, "cloudCurriculaId");
            }
        }
    }
    out
}

/// 把 ` -H `、` -b ` 等参数拆到独立行，兼容单行粘贴的 cURL。
fn split_flags(text: &str) -> String {
    let mut s = text.replace("\r\n", "\n");
    for flag in [
        " -H ",
        " --header ",
        " -b ",
        " --cookie ",
        " --url ",
        " -x ",
        " --proxy ",
        " -X ",
        " --request ",
    ] {
        s = s.replace(flag, &format!("\n{}", flag.trim_start()));
    }
    s
}

fn push_header(headers: &mut Vec<(String, String)>, line: &str) {
    if let Some((k, v)) = line.split_once(':') {
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim().to_string();
        if !k.is_empty() && !v.is_empty() {
            headers.push((k, v));
        }
    }
}

fn base64_decode_any(s: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(s.trim_end_matches('='))
        .ok()
        .or_else(|| STANDARD.decode(s).ok())
}

/// 从 cookie 的 jwt 载荷里取 openId。
pub fn cookie_open_id(cookie: &str) -> Option<String> {
    let jwt = cookie
        .split(';')
        .find_map(|p| p.trim().strip_prefix("jwt="))?;
    jwt_payload_open_id(jwt)
}

pub fn jwt_payload_open_id(jwt: &str) -> Option<String> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = base64_decode_any(payload)?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    v.get("openId")
        .or_else(|| v.get("open_id"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once(':') {
            let _ = (k, v);
        }
        if let Some((k, v)) = pair.split_once('=') {
            if k == key && !v.is_empty() {
                return Some(v.split('#').next().unwrap_or(v).to_string());
            }
        }
    }
    None
}

fn url_course_id(url: &str) -> Option<String> {
    for seg in url.split('/') {
        if seg.starts_with("course-v") && seg.contains(':') {
            return Some(seg.to_string());
        }
    }
    None
}

/// 形如 `/course/api/v2/course_progress/{course}/{open_id}/default` 里的 open_id。
fn url_open_id(url: &str) -> Option<String> {
    let parts: Vec<&str> = url.split('?').next()?.split('/').collect();
    let i = parts
        .iter()
        .position(|p| *p == "course_progress" || *p == "content")?;
    let course = parts.get(i + 1)?;
    if !course.starts_with("course-v") {
        return None;
    }
    let open = parts.get(i + 2)?;
    if open.len() >= 24 && open.chars().all(|c| c.is_ascii_hexdigit()) {
        Some((*open).to_string())
    } else {
        None
    }
}

/// 复刻前端 `_generateJwtToken()`：HS256 签发 x-annotator-auth-token。
pub fn gen_annotator_token(open_id: &str) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "open_id": open_id,
        "name": "",
        "email": "",
        "administrator": false,
        "exp": now_ms + 31_536_000_000u64,
        "iss": ANNOTATOR_ISS,
        "aud": ANNOTATOR_AUD,
    });
    let h = URL_SAFE_NO_PAD.encode(header.to_string());
    let p = URL_SAFE_NO_PAD.encode(payload.to_string());
    let signing_input = format!("{h}.{p}");
    let mut mac = Hmac::<Sha256>::new_from_slice(ANNOTATOR_SECRET.as_bytes())
        .expect("HMAC 密钥长度固定合法");
    mac.update(signing_input.as_bytes());
    let sig = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    format!("{signing_input}.{sig}")
}

/// 用 cookie 拉取「我的课程」列表（无需其他字段）。
pub async fn fetch_courses(cookie: &str) -> Result<Vec<CourseEntry>> {
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .context("构建 HTTP 客户端失败")?;
    let text = client
        .get("https://uai.unipus.cn/api/cmgt/course/getHomeCourseListByStudent")
        .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36 Edg/151.0.0.0")
        .header("accept", "application/json, text/plain, */*")
        .header("cookie", cookie)
        .header("u-app-id", "39")
        .header("u-platform", "2")
        .header("origin", "https://uai.unipus.cn")
        .header("referer", "https://uai.unipus.cn/")
        .send()
        .await
        .context("请求课程列表失败")?
        .text()
        .await?;
    let v: Value = serde_json::from_str(&text)
        .with_context(|| format!("课程列表返回非 JSON: {}", crate::api::parser::truncate_text(&text, 200)))?;
    let mut out = Vec::new();
    for c in v
        .get("value")
        .and_then(|v| v.get("courseList"))
        .and_then(|l| l.as_array())
        .unwrap_or(&Vec::new())
    {
        let class_name = c.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let class_id = c.get("classId").and_then(|x| x.as_str()).unwrap_or("");
        let curricula_id = c.get("id").map(|x| x.to_string().trim_matches('"').to_string());
        for r in c
            .get("courseResourceList")
            .and_then(|l| l.as_array())
            .unwrap_or(&Vec::new())
        {
            out.push(CourseEntry {
                class_name: class_name.to_string(),
                class_id: class_id.to_string(),
                curricula_id: curricula_id.clone().unwrap_or_default(),
                name: r.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                course_id: r
                    .get("instanceId")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                resource_id: r
                    .get("resourceId")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct CourseEntry {
    pub class_name: String,
    pub class_id: String,
    pub curricula_id: String,
    pub name: String,
    pub course_id: String,
    pub resource_id: String,
}

/// 把解析结果合并进配置（不改动未识别到的字段）。
pub fn apply_to_config(cfg: &mut Config, p: &PastedRequest) -> Vec<String> {
    let mut changed = Vec::new();
    let set = |name: &str, slot: &mut String, val: Option<String>, changed: &mut Vec<String>| {
        if let Some(v) = val
            && !v.is_empty()
            && *slot != v
        {
            *slot = v;
            changed.push(name.to_string());
        }
    };
    set("cookie", &mut cfg.cookie, p.cookie.clone(), &mut changed);
    set("authorization", &mut cfg.authorization, p.authorization.clone(), &mut changed);
    set("u_school", &mut cfg.u_school, p.u_school.clone(), &mut changed);
    set("open_id", &mut cfg.open_id, p.open_id.clone(), &mut changed);
    set("class_id", &mut cfg.class_id, p.class_id.clone(), &mut changed);
    set("curricula_id", &mut cfg.curricula_id, p.curricula_id.clone(), &mut changed);
    set("course_id", &mut cfg.course_id, p.course_id.clone(), &mut changed);

    // x-annotator-auth-token：能直接抓到就用抓到的，否则按前端算法本地生成
    let annotator = p
        .annotator
        .clone()
        .or_else(|| p.open_id.as_deref().map(gen_annotator_token));
    set("x_annotator_auth_token", &mut cfg.x_annotator_auth_token, annotator, &mut changed);
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"curl --url 'https://ucontent.unipus.cn/_explorationpc_default/pc.html?cid=1000000000000000001&theme=3264FA&cloudCurriculaId=999999&source=cloud&courseResourceId=20001356721' \
  -H 'accept: text/html' \
  -b 'WithoutLoginDeviceId=abc; jwt=eyJhbGciOiJSUzI1NiIsImtpZCI6InVuaXB1cy1zc28ifQ.eyJvcGVuSWQiOiIwMTIzNDU2Nzg5YWJjZGVmMDEyMzQ1Njc4OWFiY2RlZiJ9.sig; sessionid=xyz' \
  -H 'referer: https://ucontent.unipus.cn/'"#;

    #[test]
    fn parse_curl_paste() {
        let p = parse_pasted(SAMPLE);
        assert!(p.cookie.as_deref().unwrap().contains("jwt="));
        assert_eq!(p.class_id.as_deref(), Some("1000000000000000001"));
        assert_eq!(p.curricula_id.as_deref(), Some("999999"));
        assert_eq!(
            p.open_id.as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert!(p.course_id.is_none(), "页面 URL 里没有 course_id");
    }

    #[test]
    fn parse_header_block_and_api_url() {
        let text = "GET https://ucontent.unipus.cn/course/api/v3/content/course-v2:Unipus+nhce_v4_rw_2+20230116/abc123/default HTTP/1.1\nCookie: a=1; jwt=x.y.z\nx-annotator-auth-token: TOKEN123\n";
        // URL 行会被当成 curl 行之外的内容，这里主要验证头部解析与 URL 提取
        let p = parse_pasted(&format!("curl '{text}'"));
        assert_eq!(p.open_id, None);
        let p2 = parse_pasted(text);
        assert_eq!(p2.cookie.as_deref(), Some("a=1; jwt=x.y.z"));
        assert_eq!(p2.annotator.as_deref(), Some("TOKEN123"));
    }

    #[test]
    fn parse_cookie_only() {
        let p = parse_pasted("a=1; jwt=eyJvcGVuSWQiOiJ4eCJ9.sig");
        assert!(p.cookie.is_some());
    }

    #[test]
    fn parse_single_line_curl() {
        let text = "curl 'https://ucontent.unipus.cn/course/api/v3/content/course-v2:demo+demo+20230101/0123456789abcdef0123456789abcdef/default' -H 'cookie: jwt=eyJvcGVuSWQiOiIwMTIzNDU2Nzg5YWJjZGVmMDEyMzQ1Njc4OWFiY2RlZiJ9.sig; sessionid=x' -H 'x-annotator-auth-token: TESTTOKEN'";
        let p = parse_pasted(text);
        assert_eq!(
            p.cookie.as_deref(),
            Some("jwt=eyJvcGVuSWQiOiIwMTIzNDU2Nzg5YWJjZGVmMDEyMzQ1Njc4OWFiY2RlZiJ9.sig; sessionid=x")
        );
        assert_eq!(p.annotator.as_deref(), Some("TESTTOKEN"));
        assert_eq!(p.course_id.as_deref(), Some("course-v2:demo+demo+20230101"));
        assert_eq!(
            p.open_id.as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
    }

    #[test]
    fn parse_devtools_bash_forms() {
        // Chrome/Edge 在 Linux/mac 风格下复制出来的形式
        let bash = "curl 'https://ucontent.unipus.cn/course/api/v3/content/course-v2:Unipus+nhce_v4_rw_2+20230116/abc/default' \\\n  -H 'cookie: jwt=eyJvcGVuSWQiOiJ4eCJ9.sig' \\\n  -H 'x-annotator-auth-token: TOK'";
        let p = parse_pasted(bash);
        assert_eq!(
            p.course_id.as_deref(),
            Some("course-v2:Unipus+nhce_v4_rw_2+20230116")
        );
        assert_eq!(p.annotator.as_deref(), Some("TOK"));

        // Windows cmd 风格（^ 续行）
        let cmd = "curl \"https://ucontent.unipus.cn/course/api/v2/course_progress/course-v2:a+b+c/00000000000000000000000000000000/default\" ^\n  -H \"cookie: jwt=x\"";
        let p2 = parse_pasted(cmd);
        assert_eq!(p2.course_id.as_deref(), Some("course-v2:a+b+c"));
        assert_eq!(p2.open_id.as_deref(), Some("00000000000000000000000000000000"));
    }

    #[test]
    fn generated_token_is_valid_hs256() {
        let token = gen_annotator_token("0123456789abcdef0123456789abcdef");
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        let payload = String::from_utf8(URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert!(payload.contains("0123456789abcdef0123456789abcdef"));
        assert!(payload.contains("edx.unipus.cn"));
    }
}
