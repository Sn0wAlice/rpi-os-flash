use serde::Deserialize;
use std::fs;
use inquire::{Select, Confirm, Text};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{copy, BufReader};
use std::process::Command;
use serde_json;

#[derive(Debug)]
struct OsImage {
    name: &'static str,
    path: &'static str, // Can be a local file path or a download URL
}

#[derive(Debug, Deserialize)]
struct RemoteOs {
    name: String,
    url: String,
}

fn list_available_os() -> Vec<OsImage> {
    let json_data = fs::read_to_string("os_list.json").expect("Impossible de lire os_list.json");

    let remote_list: Vec<RemoteOs> = serde_json::from_str(&json_data).expect("Erreur de parsing JSON");

    let mut os_list: Vec<OsImage> = remote_list.iter().map(|entry| {
        OsImage {
            name: Box::leak(entry.name.clone().into_boxed_str()),
            path: Box::leak(entry.url.clone().into_boxed_str()),
        }
    }).collect();

    os_list.push(OsImage {
        name: "Custom OS (choisir un fichier local)",
        path: "custom",
    });

    os_list
}

fn fetch_os_list_json() {
    let url = "https://downloads.raspberrypi.org/os_list_v3.json";
    let response = reqwest::blocking::get(url).expect("Erreur de requête vers le dépôt d'OS");
    let body = response.text().expect("Impossible de lire la réponse");
    fs::write("os_list.json", body).expect("Impossible d'écrire os_list.json");
}

fn download_image_if_needed(image_path: &str) {
    let path = std::path::Path::new(image_path);
    if path.exists() {
        return;
    }

    println!("📥 Téléchargement de l'image depuis {}", image_path);
    let response = reqwest::blocking::get(image_path).expect("Erreur lors du téléchargement de l'image");
    let mut file = File::create(path.file_name().unwrap()).expect("Impossible de créer le fichier local");
    let content = response.bytes().expect("Impossible de lire le contenu");
    std::io::copy(&mut content.as_ref(), &mut file).expect("Impossible d'écrire l'image");
}

fn list_media_devices() -> Vec<String> {
    let output = Command::new("lsblk")
        .args(&["-o", "NAME,SIZE,TYPE,RM", "-dn"])
        .output()
        .expect("Failed to execute lsblk");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 4 && parts[2] == "disk" && parts[3] == "1" {
            let device = format!("/dev/{} - {}", parts[0], parts[1]);
            devices.push(device);
        }
    }

    devices
}

fn flash_image(image_path: &str, device: &str) -> std::io::Result<()> {
    println!("\n[!] Flashing {} to {}", image_path, device);

    let file = File::open(image_path)?;
    let metadata = file.metadata()?;
    let pb = ProgressBar::new(metadata.len());
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("=> "));

    let mut reader = BufReader::new(file);
    let mut output = File::create(device)?;
    let copied = copy(&mut reader, &mut pb.wrap_write(&mut output))?;
    pb.finish_with_message("Flash terminé !");

    println!("\n✅ {} bytes written to {}", copied, device);
    Ok(())
}

fn main() {
    fetch_os_list_json();

    println!("📦 Bienvenue dans Rust Pi Imager!");

    // 1. Choix de l'OS
    let os_list = list_available_os();
    let os_names: Vec<&str> = os_list.iter().map(|os| os.name).collect();
    let os_choice = Select::new("Choisis ton OS à flasher:", os_names).prompt().unwrap();
    let selected_os = os_list.iter().find(|os| os.name == os_choice).unwrap();

    let image_path = if selected_os.path == "custom" {
        Text::new("Entrez le chemin complet de l'image (.img):").prompt().unwrap()
    } else {
        selected_os.path.to_string()
    };

    if selected_os.path != "custom" {
        download_image_if_needed(&image_path);
    }

    // 2. Choix du support
    let media = list_media_devices();
    let selected_device = Select::new("Choisis le disque cible:", media).prompt().unwrap();
    let device_path = selected_device.split_whitespace().next().unwrap();

    // 3. Confirmation
    let confirm = Confirm::new(&format!("⚠️  Tu es sûr de vouloir flasher '{}' sur '{}'?", image_path, device_path))
        .with_default(false)
        .prompt().unwrap();

    if !confirm {
        println!("❌ Opération annulée.");
        return;
    }

    // 4. Flash
    match flash_image(&image_path, device_path) {
        Ok(_) => println!("🚀 Image flashée avec succès !"),
        Err(e) => eprintln!("❌ Erreur: {}", e),
    }
}
