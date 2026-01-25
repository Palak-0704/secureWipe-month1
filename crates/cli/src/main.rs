
use clap::Parser;
use serde::Serialize;
use clap::ValueEnum;
use simplelog::{CombinedLogger, TermLogger, WriteLogger, LevelFilter, Config, TerminalMode, ColorChoice};
use std::fs::File;
use std::path::Path;
use std::io::{self, Write};
use securewipe_core::{detect_devices, ComplianceContext, recommend_method, perform_wipe};
#[derive(Serialize)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum ExportFormat {
    Json,
    Csv,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct FeedbackRecord {
    device_id: String,
    model: String,
    recommendation: String,
    compliance_notes: Option<String>,
    explanation: String,
    feedback: String,
}

/// CLI arguments for compliance
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
            /// Enter chatbot mode after device processing
            #[arg(long, default_value_t = false)]
            chatbot: bool,
            /// Print extra device and recommendation details
            #[arg(long, default_value_t = false)]
            verbose: bool,
            /// Skip feedback prompts (batch/demo mode)
            #[arg(long, default_value_t = false)]
            no_feedback: bool,
            /// Export feedback history after run (json or csv)
            #[arg(long, value_enum, default_value_t = ExportFormat::Json)]
            export: ExportFormat,
            /// Scan and print detected devices
            #[arg(long, default_value_t = false)]
            scan_devices: bool,
    /// Enable GDPR compliance
    #[arg(long)]
    gdpr: bool,
    /// Enable HIPAA compliance
    #[arg(long)]
    hipaa: bool,
    /// Enable NIST compliance
    #[arg(long)]
    nist: bool,
    /// Custom compliance note
    #[arg(long)]
    custom: Option<String>,
    /// Chatbot model (Groq, e.g. openai/gpt-oss-120b, mixtral-8x7b-32768)
    #[arg(long, default_value = "openai/gpt-oss-120b")]
    chat_model: String,
    /// Chatbot system prompt (controls style/length)
    #[arg(long, default_value = "You are a helpful assistant for SecureWipe. Keep your answers concise (5-6 sentences) unless the user asks for more detail.")]
    system_prompt: String,
        /// List available Groq chat models and exit
        #[arg(long, default_value_t = false)]
        list_chat_models: bool,
}

#[cfg(feature = "groq_api")]
#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    // Print version and commit hash
    println!("SecureWipe CLI v{} (commit {})", env!("CARGO_PKG_VERSION"), "95333da");
    // Initialize logging to Month1-Submission/securewipe.log and to terminal
    let log_path = "data/securewipe.log";
    let ai_log_path = "data/ai_advisor.log";
    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
        WriteLogger::new(LevelFilter::Info, Config::default(), File::create(log_path).unwrap()),
        WriteLogger::new(LevelFilter::Info, Config::default(), File::create(ai_log_path).unwrap()),
    ]).unwrap();

        if cli.list_chat_models {
            println!("Available Groq chat models:");
            println!("- openai/gpt-oss-120b");
            println!("- mixtral-8x7b-32768");
            println!("- llama2-70b-4096");
            println!("- gemma-7b-it");
            println!("(Check https://console.groq.com/docs/models for the latest list.)");
            return;
        }
    log::info!("=== SecureWipe Device Detection Started ===");
    println!("\n=== SecureWipe Device Detection ===\n");
    // ...existing code...
    let devices = detect_devices();
    if devices.is_empty() {
        println!("No storage devices detected. Please check your system or permissions.");
        log::warn!("No storage devices detected.");
        return;
    }
    // Build compliance context from CLI
    let compliance = ComplianceContext {
        gdpr: cli.gdpr,
        hipaa: cli.hipaa,
        nist: cli.nist,
        custom: cli.custom.clone(),
    };

    // Prepare feedback history vector
    let mut feedback_history: Vec<FeedbackRecord> = Vec::new();

    for (i, device) in devices.iter().enumerate() {
        println!("Device [{}]", i + 1);
        println!("  Model:        {}", device.model);
        println!("  Type:         {}", device.dev_type);
        println!("  Size:         {} GB", device.size_gb);
        println!("  Serial:       {}", device.serial.as_deref().unwrap_or("N/A"));
        println!("  HPA/DCO:      {}", if device.hpa_dco { "Yes (hidden area detected)" } else { "No" });
        if !device.metadata.is_empty() {
            println!("  Metadata:");
            for (k, v) in &device.metadata {
                println!("    {}: {}", k, v);
            }
        } else {
            println!("  Metadata: (none)");
        }
        // Get AI/ML recommendation with compliance context
        let rec = recommend_method(device, Some(&compliance));
        println!("  AI/ML Recommendation: {} ({} mins, risk: {}, confidence: {:.2})", rec.method, rec.estimated_minutes, rec.risk_level, rec.confidence);
        if let Some(notes) = &rec.compliance_notes {
            println!("  Compliance Notes: {}", notes);
        }
        println!("  Explanation: {}", rec.explanation);
        if cli.verbose {
            println!("  --VERBOSE-- Full device struct: {:?}", device);
            println!("  --VERBOSE-- Full recommendation struct: {:?}", rec);
        }
        let feedback = if cli.no_feedback {
            "(skipped)".to_string()
        } else {
            print!("  Please rate this recommendation (1-5) or enter feedback: ");
            io::stdout().flush().unwrap();
            let mut feedback_buf = String::new();
            io::stdin().read_line(&mut feedback_buf).unwrap();
            let feedback_str = feedback_buf.trim().to_string();
            println!("  Your feedback: {}", feedback_str);
            log::info!("Device [{}] user feedback: {}", i + 1, feedback_str);
            feedback_str
        };
        // Store feedback record
        feedback_history.push(FeedbackRecord {
            device_id: device.id.clone(),
            model: device.model.clone(),
            recommendation: rec.method.clone(),
            compliance_notes: rec.compliance_notes.clone(),
            explanation: rec.explanation.clone(),
            feedback: feedback.to_string(),
        });
        println!("----------------------------------------");
        log::info!("Device [{}]: model={}, type={}, size={}GB, serial={:?}, hpa_dco={}, metadata={:?}, recommendation={:?}",
            i + 1, device.model, device.dev_type, device.size_gb, device.serial, device.hpa_dco, device.metadata, rec);
    }
    // Print summary table of all recommendations
    println!("\nSummary of Recommendations:");
    println!("{:<8} {:<20} {:<18} {:<8} {:<10} {:<8}", "Device", "Model", "Recommendation", "Risk", "Conf.", "Feedback");
    for (i, rec) in feedback_history.iter().enumerate() {
        println!("{:<8} {:<20} {:<18} {:<8} {:<10} {:<8}",
            i + 1,
            rec.model,
            rec.recommendation,
            rec.explanation.split('.').next().unwrap_or("").trim(),
            rec.compliance_notes.as_deref().unwrap_or("-"),
            rec.feedback);
    }

    // Export feedback history
    match cli.export {
        ExportFormat::Json => {
            let feedback_path = Path::new("data/feedback_history.json");
            if let Ok(json) = serde_json::to_string_pretty(&feedback_history) {
                if let Ok(mut file) = File::create(feedback_path) {
                    let _ = file.write_all(json.as_bytes());
                    println!("\nFeedback history saved to {:?}", feedback_path);
                    log::info!("Feedback history saved to {:?}", feedback_path);
                }
            }
        },
        ExportFormat::Csv => {
            let feedback_path = Path::new("data/feedback_history.csv");
            if let Ok(mut file) = File::create(feedback_path) {
                // Write CSV header
                let _ = writeln!(file, "device_id,model,recommendation,compliance_notes,explanation,feedback");
                for rec in &feedback_history {
                    let notes = rec.compliance_notes.as_deref().unwrap_or("").replace(",", ";");
                    let explanation = rec.explanation.replace(",", ";");
                    let feedback = rec.feedback.replace(",", ";");
                    let _ = writeln!(file, "{},{},{},{},{},{}",
                        rec.device_id, rec.model.replace(",", ";"), rec.recommendation.replace(",", ";"), notes, explanation, feedback);
                }
                println!("\nFeedback history saved to {:?}", feedback_path);
                log::info!("Feedback history saved to {:?}", feedback_path);
            }
        }
    }

    // Demo: perform wipe on first device
    if let Some(device) = devices.get(0) {
        log::info!("Performing wipe on device: {}", device.model);
        let result = perform_wipe(device);
        println!("\nWipe result for first device: {}\n", result);
        log::info!("Wipe result: {}", result);
    }
    log::info!("=== SecureWipe Device Detection Finished ===");

    // Chatbot mode integration
    if cli.chatbot {
        println!("\n=== SecureWipe Chatbot Mode (Groq API Enabled) ===");
        println!("Type your question (or 'exit' to quit):");
        loop {
            print!("You: ");
            io::stdout().flush().unwrap();
            let mut user_input = String::new();
            io::stdin().read_line(&mut user_input).unwrap();
            let user_input = user_input.trim();
            if user_input.eq_ignore_ascii_case("exit") { break; }
            let clean_input = securewipe_core::ai::sanitize_input(user_input);
            match securewipe_core::ai::chatbot_groq_api_with_config(&clean_input, &cli.chat_model, &cli.system_prompt, true).await {
                Ok(answer) => {
                    println!("Bot: {}", answer);
                    log::info!("Chatbot Q: {} | A: {}", user_input, answer);
                }
                Err(e) => {
                    println!("[Chatbot Error] {}", e);
                    log::error!("Chatbot Q: {} | ERROR: {}", user_input, e);
                }
            }
        }
    }

}



fn main() {
    let cli = Cli::parse();
    // Print version and commit hash
    println!("SecureWipe CLI v{} (commit {})", env!("CARGO_PKG_VERSION"), "95333da");
    // Initialize logging to Month1-Submission/securewipe.log and to terminal
    let log_path = "data/securewipe.log";
    let ai_log_path = "data/ai_advisor.log";
    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
        WriteLogger::new(LevelFilter::Info, Config::default(), File::create(log_path).unwrap()),
        WriteLogger::new(LevelFilter::Info, Config::default(), File::create(ai_log_path).unwrap()),
    ]).unwrap();

    if cli.scan_devices {
        println!("\n=== SecureWipe Device Detection ===");
        let devices = detect_devices();
        if devices.is_empty() {
            println!("No devices detected.");
        } else {
            for device in devices {
                println!("Device: {} | Model: {} | Type: {} | Size: {} GB", device.id, device.model, device.dev_type, device.size_gb);
                for part in device.partitions {
                    println!("  Partition: {} | Mount: {:?} | Size: {} GB", part.name, part.mount_point, part.size_gb);
                }
            }
        }
        return;
    }
    // ...existing code for other CLI features...
}

