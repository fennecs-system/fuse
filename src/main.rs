use std::{env, fs, io::{Read, Write}, net::{TcpListener, TcpStream}, path::PathBuf,
    process::{Command, Stdio}, sync::{Arc, Mutex}, thread, time::{Duration, Instant}};
use std::os::unix::{fs::PermissionsExt, process::CommandExt};

struct State { token: String, origin: String, workspace: PathBuf, job: Mutex<Job> }
#[derive(Default)]
struct Job { pid: Option<u32>, status: String }
fn authorized(got: &str, expected: &str) -> bool {
    let mut diff = got.len() ^ expected.len();
    for (a,b) in got.bytes().zip(expected.bytes()) { diff |= usize::from(a ^ b); }
    diff == 0
}
fn response(mut stream: TcpStream, code: &str, mime: &str, body: &[u8]) {
    let header = format!("HTTP/1.1 {code}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'\r\n\r\n", body.len());
    let _ = stream.write_all(header.as_bytes()); let _ = stream.write_all(body);
}
fn kill_group(pid: u32) {
    let _ = Command::new("kill").args(["-KILL", "--", &format!("-{pid}")]).status();
}
fn start(s: &Arc<State>, prompt: Vec<u8>) -> Result<(), String> {
    let mut job = s.job.lock().unwrap();
    if job.pid.is_some() { return Err("A job is already running".into()); }
    // Truncate the log before each run. stdout/stderr bypass pipe deadlock.
    let log = fs::File::create(s.workspace.join("agent.log")).map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    let mut child = Command::new("codex")
        .args(["exec", "--json", "--dangerously-bypass-approvals-and-sandbox", "-"])
        .current_dir(&s.workspace).stdin(Stdio::piped()).stdout(log).stderr(err)
        .process_group(0).spawn().map_err(|e| e.to_string())?;
    let pid = child.id();
    job.pid = Some(pid); job.status = "running".into();
    let shared = Arc::clone(s);
    thread::spawn(move || {
        // Write stdin in another thread so cancellation/timeout works if it blocks.
        if let Some(mut input) = child.stdin.take() {
            thread::spawn(move || { let _ = input.write_all(&prompt); });
        }
        let begin = Instant::now();
        let result = loop {
            match child.try_wait() {
                Ok(Some(exit)) => break format!("finished: {exit}"),
                Err(e) => { kill_group(pid); let _ = child.wait(); break format!("failed: {e}"); },
                _ => {}
            }
            let cancelled = shared.job.lock().unwrap().status == "cancelling";
            let oversized = fs::metadata(shared.workspace.join("agent.log")).map(|m| m.len() > 16*1024*1024).unwrap_or(false);
            if cancelled || oversized || begin.elapsed() > Duration::from_secs(1800) {
                kill_group(pid); let _ = child.wait();
                break if cancelled { "cancelled" } else { "stopped: time/output limit" }.into();
            }
            thread::sleep(Duration::from_millis(100));
        };
        kill_group(pid); // Clean up ordinary descendants after the harness exits.
        let mut job = shared.job.lock().unwrap(); job.pid = None; job.status = result;
    });
    Ok(())
}
fn handle(mut stream: TcpStream, s: Arc<State>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let mut bytes = Vec::new(); let mut buf = [0u8; 2048];
    let end = loop {
        if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") { break pos+4; }
        if bytes.len() > 8192 { response(stream,"431 Request Header Fields Too Large","text/plain",b"Header limit"); return; }
        match stream.read(&mut buf) { Ok(0) | Err(_) => return, Ok(n) => bytes.extend_from_slice(&buf[..n]) }
    };
    let headers = match std::str::from_utf8(&bytes[..end]) { Ok(x)=>x, Err(_)=>return };
    let mut lines = headers.split("\r\n");
    let request: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
    if request.len()!=3 || request[2]!="HTTP/1.1" { response(stream,"400 Bad Request","text/plain",b"Invalid request"); return; }
    let method=request[0].to_owned(); let path=request[1].to_owned();
    let mut length=None; let mut auth=String::new(); let mut origin=String::new();
    let mut invalid=false;
    for line in lines.filter(|l| !l.is_empty()) {
        let Some((k,v)) = line.split_once(':') else { invalid=true; continue; };
        match k.to_ascii_lowercase().as_str() {
            "content-length" => { if length.is_some() { invalid=true; } length=v.trim().parse::<usize>().ok(); if length.is_none() { invalid=true; } },
            "transfer-encoding" => invalid=true,
            "authorization" => { if !auth.is_empty() { invalid=true; } auth=v.trim().into(); },
            "origin" => { if !origin.is_empty() { invalid=true; } origin=v.trim().into(); },
            _=>{}
        }
    }
    let length=length.unwrap_or(0);
    if invalid || length>32768 { response(stream,"400 Bad Request","text/plain",b"Invalid framing or oversized body"); return; }
    if method=="GET" {
        match path.as_str() {
            "/" => { response(stream,"200 OK","text/html; charset=utf-8",include_bytes!("../web/index.html")); return; },
            "/app.js" => { response(stream,"200 OK","text/javascript",include_bytes!("../web/app.js")); return; },
            "/style.css" => { response(stream,"200 OK","text/css",include_bytes!("../web/style.css")); return; },
            _=>{}
        }
    }
    if !authorized(&auth, &format!("Bearer {}",s.token)) {
        response(stream,"401 Unauthorized","text/plain",b"Unauthorized"); return;
    }
    // Browser origins must match; CLI callers may omit Origin. No CORS support.
    if !origin.is_empty() && origin!=s.origin && origin!="http://localhost:8787" {
        response(stream,"403 Forbidden","text/plain",b"Origin rejected"); return;
    }
    while bytes.len()-end<length {
        match stream.read(&mut buf) { Ok(0)|Err(_)=>return, Ok(n)=>bytes.extend_from_slice(&buf[..n]) }
    }
    let body=&bytes[end..end+length];
    let (code, output) = match (method.as_str(),path.as_str()) {
        ("GET","/api/status") => ("200 OK",s.job.lock().unwrap().status.clone()),
        ("GET","/api/log") => {
            use std::io::{Seek, SeekFrom};
            let mut result=Vec::new();
            if let Ok(mut f)=fs::File::open(s.workspace.join("agent.log")) {
                let size=f.metadata().map(|m|m.len()).unwrap_or(0);
                let _=f.seek(SeekFrom::Start(size.saturating_sub(128*1024)));
                let _=f.take(128*1024).read_to_end(&mut result);
            }
            ("200 OK",String::from_utf8_lossy(&result).into_owned())
        },
        ("POST","/api/run") if !body.is_empty() => match start(&s,body.to_vec()) {
            Ok(())=>("202 Accepted","Started".into()), Err(e)=>("409 Conflict",e)
        },
        ("POST","/api/cancel") => {
            let mut job=s.job.lock().unwrap();
            if job.pid.is_some() { job.status="cancelling".into(); }
            ("200 OK",job.status.clone())
        },
        _=>("404 Not Found","Not found".into())
    };
    response(stream,code,"text/plain; charset=utf-8",output.as_bytes());
}
fn main() -> std::io::Result<()> {
    let token_path=PathBuf::from(env::var("AGENT_TOKEN_FILE").expect("AGENT_TOKEN_FILE required"));
    let metadata=fs::metadata(&token_path)?;
    assert_eq!(metadata.permissions().mode() & 0o077,0,"token must have mode 0600");
    let token=fs::read_to_string(token_path)?.trim().to_owned();
    assert!(token.len()>=32 && token.bytes().all(|c|c.is_ascii_alphanumeric()),"use a 32+ character random hex token");
    let workspace=PathBuf::from(env::var("AGENT_WORKSPACE").expect("AGENT_WORKSPACE required"));
    fs::create_dir_all(&workspace)?;
    let s=Arc::new(State { token, workspace, origin:env::var("AGENT_ORIGIN").unwrap_or_default(), job:Mutex::new(Job {pid:None,status:"idle".into()}) });
    let listener=TcpListener::bind(env::var("AGENT_BIND").unwrap_or("127.0.0.1:8080".into()))?;
    // Fixed pool caps unauthenticated concurrent clients and slow-reader resource use.
    let mut threads=Vec::new();
    for _ in 0..8 {
        let listener=listener.try_clone()?; let s=Arc::clone(&s);
        threads.push(thread::spawn(move || { for stream in listener.incoming().flatten() { handle(stream,Arc::clone(&s)); } }));
    }
    for t in threads { let _=t.join(); } Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn tokens_require_exact_match() {
        assert!(authorized("abc","abc")); assert!(!authorized("abc","abcd"));
        assert!(!authorized("abd","abc")); assert!(!authorized("","abc"));
    }
}
