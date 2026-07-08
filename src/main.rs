//! synth-cli — terminal front-end for the synth modular synthesizer platform.

use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, Subcommand};
use synth_core::model::{ParamValue, Patch};
use synth_core::module::{OsPermission, Registry};
use synth_core::plan_engine::{EngineError, PlanEngine};

/// Maximum audio block size (frames) the engine pre-allocates for.
const MAX_FRAMES: usize = 16384;

#[derive(Parser)]
#[command(name = "synth-cli", about = "synth modular synthesizer — CLI")]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Run a patch (.yml): build the engine and play it on the default audio device.
    Run {
        /// Input patch YAML.
        input: PathBuf,
    },
    /// Render a patch (.yml) offline to a WAV file.
    Render {
        /// Input patch YAML.
        input: PathBuf,
        /// Output WAV file.
        output: PathBuf,
        /// Seconds of audio to generate.
        seconds: f64,
    },
    /// Draw a patch (.yml) as a graph image via Graphviz.
    Graph {
        /// Input patch YAML.
        input: PathBuf,
        /// Output image; format is inferred from the extension (e.g. .png, .svg).
        output: PathBuf,
    },
}

fn main() {
    // Default to `info`; override with e.g. `RUST_LOG=synth_core::audio=trace` for per-callback
    // audio tracing. Logs go to stderr, keeping stdout clean for the CLI's own output.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if let Err(e) = try_main() {
        use std::io::IsTerminal;
        let tty = std::io::stderr().is_terminal();
        // Downcast to EngineError for structured formatting; fall back to Display for anything else.
        let msg = if let Some(ee) = e.downcast_ref::<EngineError>() {
            engine_error_message(ee)
        } else {
            e.to_string()
        };
        print_error(&msg, tty);
        std::process::exit(1);
    }
}

/// Compose a human-readable, CLI-appropriate error message for an [`EngineError`].
/// This is the only place in synth-cli that knows how to phrase engine errors for a terminal user.
fn engine_error_message(e: &EngineError) -> String {
    match e {
        EngineError::PermissionDenied { node, permission } => {
            match permission {
                OsPermission::Accessibility => {
                    let app = terminal_app_name();
                    let url = "x-apple.systempreferences:\
                               com.apple.preference.security?Privacy_Accessibility";
                    format!(
                        "node '{node}': requires Accessibility permission for {app}\n\
                         {url}\n\
                         add {app} to the list, then restart"
                    )
                }
            }
        }
        other => other.to_string(),
    }
}

/// Best-effort name of the terminal emulator, read from the standard `TERM_PROGRAM` env var.
fn terminal_app_name() -> &'static str {
    match std::env::var("TERM_PROGRAM").as_deref() {
        Ok("Apple_Terminal") => "Terminal",
        Ok("iTerm.app") => "iTerm2",
        Ok("WezTerm") => "WezTerm",
        Ok("vscode") => "Visual Studio Code",
        Ok("Hyper") => "Hyper",
        _ => "your terminal",
    }
}

/// Print a (possibly multi-line) message with an `error:` prefix.  Continuation lines are
/// indented to align.  In tty mode bare URLs become OSC 8 hyperlinks (clickable in iTerm2,
/// Terminal.app Ventura+, WezTerm, kitty).
fn print_error(msg: &str, tty: bool) {
    let (red, bold, reset) = if tty {
        ("\x1b[31m", "\x1b[1m", "\x1b[0m")
    } else {
        ("", "", "")
    };
    const PREFIX: &str = "error: ";
    const INDENT: &str = "       "; // same visual width as PREFIX
    let mut lines = msg.lines();
    if let Some(first) = lines.next() {
        eprintln!("{bold}{red}{PREFIX}{reset}{first}");
    }
    for line in lines {
        let line = if tty { osc8_if_url(line) } else { line.to_string() };
        eprintln!("{INDENT}{line}");
    }
}

/// Wrap a bare URL in an OSC 8 hyperlink so the terminal renders it as a clickable link.
fn osc8_if_url(s: &str) -> String {
    let is_url = s.starts_with("x-apple.systempreferences:")
        || s.starts_with("https://")
        || s.starts_with("http://");
    if is_url {
        format!("\x1b]8;;{s}\x1b\\{s}\x1b]8;;\x1b\\")
    } else {
        s.to_string()
    }
}

fn try_main() -> Result<(), Box<dyn Error>> {
    match Cli::parse().command {
        CliCommand::Run { input } => run(&input),
        CliCommand::Render {
            input,
            output,
            seconds,
        } => render(&input, &output, seconds),
        CliCommand::Graph { input, output } => graph(&input, &output),
    }
}

/// Apply host-level overrides before building the engine. The mode of `ansi_keyboard` is a host
/// decision (kept out of the patch file so the same patch loads in any host): the CLI uses the
/// backtick on/off toggle, so it sets `toggle: true`.
fn cli_patch(mut patch: Patch) -> Patch {
    for node in &mut patch.nodes {
        if node.ty == "ansi_keyboard" {
            node.params.insert("toggle".to_string(), ParamValue::Bool(true));
        }
    }
    patch
}

/// Override the `audio_output` node's `sample_rate` so the engine generates at `rate`.
fn set_output_sample_rate(patch: &mut Patch, rate: u32) {
    for node in &mut patch.nodes {
        if node.ty == "audio_output" {
            node.params
                .insert("sample_rate".to_string(), ParamValue::Int(rate as i64));
        }
    }
}

fn render(input: &Path, output: &Path, seconds: f64) -> Result<(), Box<dyn Error>> {
    let yaml = std::fs::read_to_string(input)
        .map_err(|e| format!("reading {}: {e}", input.display()))?;
    let patch = cli_patch(Patch::from_yaml(&yaml)?);
    let engine = PlanEngine::build(&patch, &Registry::with_builtins(), MAX_FRAMES)?;

    let sample_rate = engine.sample_rate();
    let channels = engine.channels();
    synth_core::wav::render_to_wav(engine, output, seconds)?;

    println!(
        "rendered {seconds}s to {} ({sample_rate} Hz, {channels} ch)",
        output.display()
    );
    Ok(())
}

fn run(input: &Path) -> Result<(), Box<dyn Error>> {
    let yaml = std::fs::read_to_string(input)
        .map_err(|e| format!("reading {}: {e}", input.display()))?;
    let mut patch = cli_patch(Patch::from_yaml(&yaml)?);

    // Run the engine at the output device's native sample rate. Forcing the patch's rate onto a
    // device with a different native rate drives some ALSA backends (Raspberry Pi) into a broken
    // resample path that plays one buffer and then stalls silently.
    if let Some(dev_rate) = synth_core::audio::default_output_sample_rate() {
        set_output_sample_rate(&mut patch, dev_rate);
    }

    let engine = PlanEngine::build(&patch, &Registry::with_builtins(), MAX_FRAMES)?;

    let sample_rate = engine.sample_rate();
    let channels = engine.channels();
    let _stream = synth_core::audio::run_default_output(engine)?;

    println!(
        "playing {} at {sample_rate} Hz, {channels} ch — press Ctrl-C to stop",
        input.display()
    );
    loop {
        std::thread::park();
    }
}

fn graph(input: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let yaml = std::fs::read_to_string(input)
        .map_err(|e| format!("reading {}: {e}", input.display()))?;
    let patch = Patch::from_yaml(&yaml)?;
    run_graphviz(&to_dot(&patch), output)?;
    println!("wrote {}", output.display());
    Ok(())
}

/// Render the patch's top-level nodes and wires as a Graphviz DOT graph.
fn to_dot(patch: &Patch) -> String {
    let mut s = String::from("digraph patch {\n  rankdir=LR;\n  node [shape=box, style=rounded];\n");
    for node in &patch.nodes {
        let label = format!("{}\n{}", node.id, node.ty);
        s.push_str(&format!("  {} [label={}];\n", dot_quote(&node.id), dot_quote(&label)));
    }
    for w in &patch.wires {
        let label = format!("{} → {}", w.from.port(), w.to.port());
        s.push_str(&format!(
            "  {} -> {} [label={}];\n",
            dot_quote(w.from.node()),
            dot_quote(w.to.node()),
            dot_quote(&label),
        ));
    }
    s.push_str("}\n");
    s
}

/// Quote a string as a DOT identifier/label, escaping specials and turning newlines into `\n`.
fn dot_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Pipe DOT into `dot` and write the rendered image to `output`.
fn run_graphviz(dot: &str, output: &Path) -> Result<(), Box<dyn Error>> {
    let format = output.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let mut child = Command::new("dot")
        .arg(format!("-T{format}"))
        .arg("-o")
        .arg(output)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run Graphviz `dot` (is graphviz installed?): {e}"))?;
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(dot.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("graphviz `dot` failed ({status})").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_contains_nodes_and_wires() {
        let yaml = "nodes:\n  - id: a\n    type: const_generator\n  - id: b\n    type: audio_output\nwires:\n  - { from: [a, out], to: [b, ch0] }\n";
        let patch = Patch::from_yaml(yaml).unwrap();
        let dot = to_dot(&patch);
        assert!(dot.starts_with("digraph patch {"));
        assert!(dot.contains("\"a\" [label=\"a\\nconst_generator\"]"));
        assert!(dot.contains("\"a\" -> \"b\" [label=\"out → ch0\"]"));
    }

    #[test]
    fn dot_quote_escapes() {
        assert_eq!(dot_quote("x\"y\\z"), "\"x\\\"y\\\\z\"");
        assert_eq!(dot_quote("a\nb"), "\"a\\nb\"");
    }

    #[test]
    fn set_output_sample_rate_overrides_audio_output_node() {
        let yaml = "nodes:\n  - id: a\n    type: const_generator\n  - id: out\n    type: audio_output\n    params: { sample_rate: 44100, channels: 2 }\nwires:\n  - { from: [a, out], to: [out, ch0] }\n";
        let mut patch = Patch::from_yaml(yaml).unwrap();
        set_output_sample_rate(&mut patch, 48000);
        let out = patch.nodes.iter().find(|n| n.ty == "audio_output").unwrap();
        assert_eq!(out.params.get("sample_rate"), Some(&ParamValue::Int(48000)));
        // Non-sink nodes are untouched.
        let a = patch.nodes.iter().find(|n| n.id == "a").unwrap();
        assert!(!a.params.contains_key("sample_rate"));
    }
}
