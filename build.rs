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

/// Copy every regular file from `from_dir` into `to_dir` (non-recursive).
fn copy_dir_files(from_dir: &str, to_dir: &str) {
    let from = Path::new(from_dir);
    let to = Path::new(to_dir);
    fs::create_dir_all(to).unwrap_or_else(|e| panic!("Failed to create {to_dir}: {e}"));
    let entries =
        fs::read_dir(from).unwrap_or_else(|e| panic!("Failed to read dir {from_dir}: {e}"));
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap();
            let dest = to.join(filename);
            fs::copy(&path, &dest)
                .unwrap_or_else(|e| panic!("Failed to copy {}: {e}", path.display()));
        }
    }
}

fn main() {
    // --- Git commit hash ---
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output();

    let git_hash = match output {
        Ok(o) if o.status.success() => {
            let hash = String::from_utf8(o.stdout)
                .unwrap_or_default()
                .trim()
                .to_owned();
            if hash.is_empty() { "unknown".to_owned() } else { hash }
        }
        _ => "unknown".to_owned(),
    };

    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");

    // --- CSS/JS asset pipeline ---
    println!("cargo:rerun-if-changed=assets/static/style.css");
    println!("cargo:rerun-if-changed=assets/static/script.js");
    println!("cargo:rerun-if-changed=assets/src/player.js");
    println!("cargo:rerun-if-changed=assets/src/jassub-loader.js");
    println!("cargo:rerun-if-changed=templates/");
    println!("cargo:rerun-if-changed=purgecss.config.cjs");

    let processed_dir = Path::new("assets/processed");
    fs::create_dir_all(processed_dir).expect("Failed to create assets/processed directory");

    // Install npm dependencies if node_modules missing
    if !Path::new("node_modules/.package-lock.json").exists() {
        println!("cargo:warning=Installing npm dependencies for CSS/JS optimization...");
        run_cmd("npm", &["install", "--ignore-scripts"], "npm install");
    }

    // Step 1: Copy vidstack CSS verbatim from node_modules — no esbuild transformation.
    // esbuild's CSS bundling can silently strip or mangle rules it does not understand
    // (complex :has(), @layer, mask-image data-URLs, etc.), breaking the player theme.
    // A direct file concatenation guarantees the CSS is exactly what vidstack ships.
    println!("cargo:warning=Copying vidstack CSS from node_modules...");
    {
        let theme = fs::read_to_string(
            "node_modules/vidstack/player/styles/default/theme.css",
        ).expect("Failed to read node_modules/vidstack/player/styles/default/theme.css");
        let video = fs::read_to_string(
            "node_modules/vidstack/player/styles/default/layouts/video.css",
        ).expect("Failed to read node_modules/vidstack/player/styles/default/layouts/video.css");
        fs::write("assets/processed/player.css", format!("{}\n{}", theme, video))
            .expect("Failed to write assets/processed/player.css");
    }

    // Step 2: Bundle vidstack player JS with esbuild (ESM format, no CSS imports).
    println!("cargo:warning=Bundling vidstack player with esbuild...");
    run_cmd(
        "npx",
        &[
            "esbuild",
            "assets/src/player.js",
            "--bundle",
            "--format=esm",
            "--platform=browser",
            "--target=es2020",
            "--minify",
            // /jassub/jassub.js is a runtime URL served from assets/processed/jassub/;
            // mark it external so esbuild does not try to resolve it as a file path.
            "--external:/jassub/jassub.js",
            "--outfile=assets/processed/player.js",
        ],
        "esbuild player bundle",
    );

    // Step 3: Bundle jassub as a lazy-loaded ESM module.
    // Served at /jassub/jassub.js and loaded on demand via dynamic import().
    println!("cargo:warning=Bundling jassub with esbuild...");
    fs::create_dir_all("assets/processed/jassub")
        .expect("Failed to create assets/processed/jassub directory");
    run_cmd(
        "npx",
        &[
            "esbuild",
            "assets/src/jassub-loader.js",
            "--bundle",
            "--format=esm",
            "--platform=browser",
            "--target=es2020",
            "--minify",
            "--outfile=assets/processed/jassub/jassub.js",
        ],
        "esbuild jassub bundle",
    );

    // Step 4: Copy jassub worker scripts and WASM from node_modules so they are
    // served at /jassub/jassub-worker.js and /jassub/jassub-worker-legacy.js.
    // The worker loads its WASM file relative to its own URL, so all dist files
    // must live together in the same directory.
    println!("cargo:warning=Copying jassub worker files...");
    copy_dir_files("node_modules/jassub/dist", "assets/processed/jassub");

    // Step 5: PurgeCSS — remove unused CSS by scanning templates
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

    // Step 6: Minify CSS with csso
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

    // Step 7: Minify JS with terser
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
    if let Ok(player_js) = fs::metadata("assets/processed/player.js") {
        println!(
            "cargo:warning=player.js: {:.1} KB (bundled vidstack)",
            player_js.len() as f64 / 1024.0
        );
    }
    if let Ok(player_css) = fs::metadata("assets/processed/player.css") {
        println!(
            "cargo:warning=player.css: {:.1} KB (bundled vidstack CSS)",
            player_css.len() as f64 / 1024.0
        );
    }
}
