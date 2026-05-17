use std::fs;
use std::path::Path;
use std::process::Command;

fn run_cmd(program: &str, args: &[&str], description: &str) {
    let status = Command::new(program)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("Failed to run {description}: {e}"));
    if !status.success() {
        panic!("{description} failed with exit code: {status}");
    }
}

fn command_exists(program: &str) -> bool {
    Command::new(program).arg("--version").output().is_ok()
}

fn main() {
    // --- Git commit hash ---
    // Prefer value injected via Docker build-arg / environment variable (set during CI builds
    // where git is not available inside the container).
    let git_hash = std::env::var("GIT_COMMIT_HASH")
        .ok()
        .filter(|v| !v.is_empty() && v != "unknown")
        .unwrap_or_else(|| {
            let output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    let hash = String::from_utf8(o.stdout)
                        .unwrap_or_default()
                        .trim()
                        .to_owned();
                    if hash.is_empty() { "unknown".to_owned() } else { hash }
                }
                _ => "unknown".to_owned(),
            }
        });

    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);

    // --- Git branch ---
    let git_branch = std::env::var("GIT_BRANCH")
        .ok()
        .filter(|v| !v.is_empty() && v != "unknown")
        .unwrap_or_else(|| {
            let branch_output = Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output();
            match branch_output {
                Ok(o) if o.status.success() => {
                    let branch = String::from_utf8(o.stdout)
                        .unwrap_or_default()
                        .trim()
                        .to_owned();
                    if branch.is_empty() { "unknown".to_owned() } else { branch }
                }
                _ => "unknown".to_owned(),
            }
        });

    println!("cargo:rustc-env=GIT_BRANCH={}", git_branch);
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");

    // --- CSS/JS asset pipeline ---
    println!("cargo:rerun-if-changed=assets/static/style.css");
    println!("cargo:rerun-if-changed=assets/static/script.js");
    println!("cargo:rerun-if-changed=templates/");
    println!("cargo:rerun-if-changed=purgecss.config.cjs");
    println!("cargo:rerun-if-changed=locales/");

    let processed_dir = Path::new("assets/processed");
    fs::create_dir_all(processed_dir).expect("Failed to create assets/processed directory");

    let npm_available = command_exists("npm");
    let npx_available = command_exists("npx");

    if !npm_available || !npx_available {
        println!(
            "cargo:warning=Skipping CSS/JS optimization because npm or npx is unavailable"
        );

        fs::copy("assets/static/style.css", "assets/processed/style.css")
            .expect("Failed to copy fallback CSS asset");
        fs::copy("assets/static/script.js", "assets/processed/script.js")
            .expect("Failed to copy fallback JS asset");
    } else {
        // Install npm dependencies if node_modules missing
        if !Path::new("node_modules/.package-lock.json").exists() {
            println!("cargo:warning=Installing npm dependencies for CSS/JS optimization...");
            run_cmd("npm", &["install", "--ignore-scripts"], "npm install");
        }

        // Step 1: PurgeCSS — remove unused CSS by scanning templates
        println!("cargo:warning=Running PurgeCSS...");
        run_cmd(
            "npx",
            &[
                "purgecss",
                "--config", "purgecss.config.cjs",
                "--output", "assets/processed",
            ],
            "PurgeCSS",
        );

        // Step 2: Minify CSS with csso
        println!("cargo:warning=Minifying CSS...");
        run_cmd(
            "npx",
            &[
                "csso",
                "assets/processed/style.css",
                "--output", "assets/processed/style.css",
            ],
            "csso CSS minification",
        );

        // Step 3: Minify JS with terser
        println!("cargo:warning=Minifying JS...");
        run_cmd(
            "npx",
            &[
                "terser",
                "assets/static/script.js",
                "--compress",
                "--mangle",
                "--output", "assets/processed/script.js",
            ],
            "terser JS minification",
        );
    }

    // Report sizes
    if let (Ok(orig_css), Ok(new_css)) = (
        fs::metadata("assets/static/style.css"),
        fs::metadata("assets/processed/style.css"),
    ) {
        let orig = orig_css.len();
        let processed = new_css.len();
        let reduction = if orig > 0 {
            (1.0 - processed as f64 / orig as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "cargo:warning=CSS: {:.1} KB -> {:.1} KB ({:.1}% reduction)",
            orig as f64 / 1024.0,
            processed as f64 / 1024.0,
            reduction
        );
    }
    if let (Ok(orig_js), Ok(new_js)) = (
        fs::metadata("assets/static/script.js"),
        fs::metadata("assets/processed/script.js"),
    ) {
        let orig = orig_js.len();
        let processed = new_js.len();
        let reduction = if orig > 0 {
            (1.0 - processed as f64 / orig as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "cargo:warning=JS:  {:.1} KB -> {:.1} KB ({:.1}% reduction)",
            orig as f64 / 1024.0,
            processed as f64 / 1024.0,
            reduction
        );
    }

    memory_serve::load_directory("assets/processed");
}
