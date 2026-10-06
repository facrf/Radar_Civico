use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=APP_VERSION");
    println!("cargo:rerun-if-env-changed=APP_COMMIT");
    println!("cargo:rerun-if-env-changed=../../version.json");
    if Path::new("../../.git/HEAD").exists() {
        println!("cargo:rerun-if-changed=../../.git/HEAD");
    }

    // Variáveis de ambiente com limpeza de strings vazias
    let mut version = std::env::var("APP_VERSION")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let mut commit = std::env::var("APP_COMMIT")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let mut count = std::env::var("APP_COUNT")
        .ok()
        .and_then(|c| c.trim().parse::<u64>().ok())
        .filter(|&c| c > 0);

    // 1. Tentar ler do Git caso não fornecido por variável de ambiente
    if version.is_none() {
        let git_count_out = Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .output();

        if let Ok(out) = git_count_out {
            if out.status.success() {
                let cnt_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if let Ok(c) = cnt_str.parse::<u64>() {
                    count = Some(c);
                    version = Some(if c < 1000 {
                        format!("v0.{:03}", c)
                    } else {
                        format!("v0.{}", c)
                    });
                }
            }
        }
    }

    if commit.is_none() {
        let git_commit_out = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output();

        if let Ok(out) = git_commit_out {
            if out.status.success() {
                let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !hash.is_empty() {
                    commit = Some(hash);
                }
            }
        }
    }

    // 2. Fallback: tentar ler do arquivo version.json se existir
    if version.is_none() || commit.is_none() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let from_manifest = Path::new(&manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("version.json"));

        let candidate_paths = [
            from_manifest.as_deref(),
            Some(Path::new("../../version.json")),
            Some(Path::new("../version.json")),
            Some(Path::new("version.json")),
            Some(Path::new("/app/version.json")),
        ];

        for opt_p in candidate_paths.into_iter().flatten() {
            if opt_p.exists() {
                if let Ok(content) = std::fs::read_to_string(opt_p) {
                    if let Ok(v) = serde_json_lite(&content) {
                        if version.is_none() && !v.0.is_empty() {
                            version = Some(v.0);
                        }
                        if commit.is_none() && !v.1.is_empty() {
                            commit = Some(v.1);
                        }
                        if count.is_none() && v.2 > 0 {
                            count = Some(v.2);
                        }
                        break;
                    }
                }
            }
        }
    }

    let final_version = version.unwrap_or_else(|| "v0.001".to_string());
    let final_commit = commit.unwrap_or_else(|| "unknown".to_string());
    let final_count = count.unwrap_or(1);

    println!("cargo:rustc-env=APP_VERSION={}", final_version);
    println!("cargo:rustc-env=APP_GIT_COMMIT={}", final_commit);
    println!("cargo:rustc-env=APP_COMMIT_COUNT={}", final_count);
}

// Parser simples sem adicionar dependência pesada no build-dependencies
fn serde_json_lite(json: &str) -> Result<(String, String, u64), ()> {
    let mut version = String::new();
    let mut commit = String::new();
    let mut count = 0u64;

    for line in json.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"version\"") {
            if let Some(val) = extract_string_value(trimmed) {
                version = val;
            }
        } else if trimmed.starts_with("\"commit\"") {
            if let Some(val) = extract_string_value(trimmed) {
                commit = val;
            }
        } else if trimmed.starts_with("\"count\"") {
            if let Some(val) = extract_number_value(trimmed) {
                count = val;
            }
        }
    }

    if !version.is_empty() {
        Ok((version, commit, count))
    } else {
        Err(())
    }
}

fn extract_string_value(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() >= 2 {
        let val = parts[1].trim().trim_matches(|c| c == ',' || c == '"' || c == ' ' || c == '\r');
        return Some(val.to_string());
    }
    None
}

fn extract_number_value(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() >= 2 {
        let val = parts[1].trim().trim_matches(|c| c == ',' || c == ' ' || c == '\r');
        return val.parse().ok();
    }
    None
}
