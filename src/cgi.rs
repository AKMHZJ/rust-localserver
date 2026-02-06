use std::process::{Command, Stdio};
use std::io::Write;
use std::collections::HashMap;

pub struct CgiHandler {
    pub script_path: String,
    pub interpreter: String,
}

impl CgiHandler {
    pub fn new(script_path: String, interpreter: String) -> Self {
        CgiHandler { script_path, interpreter }
    }

    pub fn execute(&self, env_vars: HashMap<String, String>, body: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut child = Command::new(&self.interpreter)
            .arg(&self.script_path)
            .envs(env_vars)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if !body.is_empty() {
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(body)?;
        }

        let output = child.wait_with_output()?;
        
        if output.status.success() {
            Ok(output.stdout)
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(std::io::Error::new(std::io::ErrorKind::Other, err))
        }
    }
}
