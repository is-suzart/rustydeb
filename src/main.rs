use ar::Archive;
use std::fs::{File};
use std::fs;
use tar::Archive as TarArchive;
use flate2::read::GzDecoder;   // Para descompressão de .gz
use xz2::read::XzDecoder;      // Para descompressão de .xz
use std::env;
use std::path::{Path, PathBuf};
use std::process;
use tempfile::TempDir;
use std::io::Read;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)] // Debug para facilitar a impressão
struct FileManifestEntry {
    path: String,       // Caminho relativo do arquivo, ex: "usr/bin/meu_app"
    can_remove: bool,   // Para nossa lógica simplificada inicial
    // checksum: Option<String>, // Futuramente, poderíamos adicionar o checksum daqui
}

#[derive(Serialize, Deserialize, Debug)]
struct PackageInstallInfo {
    name: String,
    version: String,
    architecture: Option<String>, // Podemos pegar mais campos do 'control'
    description: Option<String>,
    files: Option<Vec<FileManifestEntry>>,
    #[serde(rename = "installFinished")] // Para ter o nome exato no JSON
    install_finished: bool,
}

fn get_deb_file(path: &str){
    let path_obj = Path::new(path);
    if !path_obj.exists(){
        eprintln!("Error: The specified file '{}' was not found.", path);
        process::exit(2); // Exit the program with an error code
    }
    if !path_obj.is_file() {
        eprintln!("Error: The specified path '{}' is not a file.", path);
        process::exit(3);
    }
    match path_obj.extension().and_then(|os_str| os_str.to_str()) {
        Some("deb") => {
            handle_deb_package(path);
        }
        _ => {
            eprintln!( // TODO: Checksum
                "Error: The file '{}' does not appear to be a valid .deb file (incorrect extension).",
                path
            );
            process::exit(1);
        }
    }
}

fn extract_tar(tar_files_to_extract: Vec<PathBuf>, base_temp_dir: &Path) -> Result<(), String> {
    println!("\nIniciando extração dos arquivos .tar coletados para subdiretórios individuais...");

    if tar_files_to_extract.is_empty() {
        println!("Nenhum arquivo .tar foi fornecido na lista para extração.");
        return Ok(());
    }

    for tar_file_path in tar_files_to_extract {
        // tar_file_path é o caminho completo, ex: /tmp/rustydeb_temp_XYZ/control.tar.xz
        let filename_osstr = tar_file_path.file_name().unwrap_or_default();
        let filename_str_cow = filename_osstr.to_string_lossy(); // ex: "control.tar.xz"

        println!("Processando arquivo tar: {}", filename_str_cow);

        // Derivar o nome do subdiretório a partir do nome do arquivo tar.
        // Queremos "control" de "control.tar.xz", "data" de "data.tar.gz", etc.
        let subdir_name_str: String;

        if filename_str_cow.starts_with("control.tar") {
            subdir_name_str = "control".to_string();
        } else if filename_str_cow.starts_with("data.tar") {
            subdir_name_str = "data".to_string();
        } else {
            // Fallback para outros nomes de .tar, pegando a parte antes do primeiro '.'
            // Isso pode ser útil se houver outros arquivos .tar inesperados.
            subdir_name_str = filename_str_cow.split('.').next().unwrap_or("unknown_tar_content").to_string();
            eprintln!(
                "Aviso: Nome de arquivo tar não é 'control.tar.*' nem 'data.tar.*': '{}'. Usando subdiretório: '{}'",
                filename_str_cow, subdir_name_str
            );
        }
        
        // Criar o subdiretório de destino específico (ex: .../control/ ou .../data/)
        let specific_destination_dir = base_temp_dir.join(&subdir_name_str);
        
        if let Err(e) = fs::create_dir_all(&specific_destination_dir) {
            return Err(format!(
                "Falha ao criar subdiretório de destino '{}': {}",
                specific_destination_dir.display(), e
            ));
        }

        println!(
            "  Extraindo conteúdo de '{}' para '{}'",
            filename_str_cow,
            specific_destination_dir.display()
        );

        // Chamar a função auxiliar para fazer a extração do tarball atual
        // para o seu subdiretório específico.
        if let Err(e) = extract_single_tar_archive(&tar_file_path, &specific_destination_dir) {
            return Err(format!(
                "Falha ao extrair o arquivo tar '{}': {}",
                filename_str_cow, e
            ));
        }
        println!(
            "  Conteúdo de '{}' extraído com sucesso para '{}'.",
            filename_str_cow, specific_destination_dir.display()
        );
    }

    println!("\nExtração de todos os arquivos .tar da lista para seus respectivos subdiretórios concluída.");
    load_control_file(base_temp_dir);
    Ok(())
}

fn load_control_file(tmp_path: &Path) -> Result<HashMap<String, String>, String>  {
    let control = tmp_path.join("control/control");
    let content = fs::read_to_string(&control).map_err(|e| {
        format!("Falha ao ler o arquivo '{}': {}", control.display(), e)
    })?;

    let mut control_data = HashMap::new();
    let mut current_key: Option<String> = None;
    let mut current_value: String = String::new();

    for line in content.lines() {
        if line.starts_with(' ') { // Linha de continuação
            if current_key.is_some() {
                current_value.push('\n'); // Adiciona quebra de linha da continuação (opcional)
                current_value.push_str(line.trim_start());
            }
        } else if let Some(colon_idx) = line.find(':') { // Nova chave: valor
            // Salvar o valor anterior, se houver
            if let Some(key) = current_key.take() {
                control_data.insert(key, current_value.trim().to_string());
            }
            current_key = Some(line[..colon_idx].to_string());
            current_value = line[colon_idx + 1..].trim_start().to_string();
        }
    }
    // Salvar o último par chave/valor
    if let Some(key) = current_key {
        control_data.insert(key, current_value.trim().to_string());
    }
    Ok(control_data)

}

fn extract_single_tar_archive(tarball_path: &Path, destination_dir: &Path) -> Result<(), String> {
    println!(
        "  Auxiliar: Iniciando extração de '{}' para '{}'",
        tarball_path.display(),
        destination_dir.display()
    );

    let tar_file_reader = File::open(tarball_path).map_err(|e| {
        format!("Falha ao abrir o arquivo tar '{}': {}", tarball_path.display(), e)
    })?;

    // Determinar o tipo de compressão pelo nome do arquivo
    let file_name_str = tarball_path.file_name().unwrap_or_default().to_string_lossy();
    
    let decompressed_reader: Box<dyn Read> = if file_name_str.ends_with(".tar.gz") {
        println!("    -> Detectado .tar.gz, usando GzDecoder.");
        Box::new(GzDecoder::new(tar_file_reader))
    } else if file_name_str.ends_with(".tar.xz") {
        println!("    -> Detectado .tar.xz, usando XzDecoder.");
        Box::new(XzDecoder::new(tar_file_reader))
    } else if file_name_str.ends_with(".tar") { // Caso seja um .tar não comprimido
        println!("    -> Detectado .tar (não comprimido).");
        Box::new(tar_file_reader)
    } else {
        return Err(format!(
            "Formato de compressão não suportado ou extensão desconhecida para o arquivo '{}'",
            tarball_path.display()
        ));
    };

    // Agora, crie o TarArchive com o fluxo JÁ DESCOMPRIMIDO (on-the-fly)
    let mut archive = TarArchive::new(decompressed_reader);
    
    println!(
        "    -> Extraindo conteúdo de tar para '{}'",
        destination_dir.display()
    );
    if let Err(e) = archive.unpack(destination_dir) { // Extrai para o diretório de destino
        return Err(format!(
            "Falha ao extrair o conteúdo de '{}' para '{}': {}",
            tarball_path.display(),
            destination_dir.display(),
            e
        ));
    }
    Ok(())
}

fn handle_deb_package(path: &str) {
    let mut tar_files: Vec<PathBuf> = Vec::new();
    eprintln!("Processing .deb file: {}", path);
    match TempDir::new(){
        Ok(temp_dir) => {
            println!("Temporary directory created at: {:?}", temp_dir.path());
            
            let file_reader = match File::open(path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error opening file '{}': {}", path, e);
                    process::exit(1);
                }
            };
            
            let mut archive = Archive::new(file_reader);
            while let Some(entry_result) = archive.next_entry() {
                let mut entry = entry_result.unwrap(); // TODO: Handle this unwrap more gracefully
                let header = entry.header();
                let entry_filename_bytes = header.identifier();

                // This is a borrowed &str, its lifetime is tied to `entry`
                let entry_filename_slice = match std::str::from_utf8(entry_filename_bytes) {
                    Ok(s) => s.trim(), // Trim to remove potential whitespace
                    Err(e) => {
                        eprintln!("Warning: Non-UTF-8 filename {:?}: {}. Skipping.", entry_filename_bytes, e);
                        continue; // Skip this entry if the name is not valid UTF-8
                    }
                };
                if entry_filename_slice.is_empty() {
                    eprintln!("Aviso: Entrada com nome de arquivo vazio encontrada. Pulando.");
                    continue;
                }
                
                // Convert the borrowed slice to an owned String
                // This decouples its lifetime from `entry`
                let entry_filename_owned: String = entry_filename_slice.to_string();
                
                let destination_path = temp_dir.path().join(&entry_filename_owned); // Use the owned string
                let mut dest_file = match std::fs::File::create(&destination_path) {
                    Ok(file) => file,
                    Err(e) => {
                        eprintln!("Error creating destination file '{}': {}", destination_path.display(), e);
                        process::exit(1);
                    }
                };
                if let Err(e) = std::io::copy(&mut entry, &mut dest_file) {
                    eprintln!(
                        "Error copying data from entry '{}' to '{}': {}",
                        entry_filename_owned, // Use the owned string for error messages too
                        destination_path.display(),
                        e
                    );
                    // Decide how to handle the error
                    process::exit(1); // For now, exit. Consider other error handling strategies.
                }
                if entry_filename_owned.contains(".tar"){
                    tar_files.push(destination_path);
                }


            // if let Err(err_msg) = handle_deb_installation(package_path_str, &temp_dir) {
            //     eprintln!("Erro durante o processamento do pacote: {}", err_msg);
            //     process::exit(1);
            // }
            }
            extract_tar(tar_files, temp_dir.path());
    }
    Err(e) => {
        eprintln!("Error creating temporary directory: {}", e);
        process::exit(1);
        
    }
    }
    
}

fn main() {

    println!("Welcome to RustyDeb!");
    let args: Vec<String> = env::args().collect();
    eprintln!("Args: {:?}", args);
    if args.len() <2 {
        eprintln!("Usage: rustydeb <path_to_deb_file.deb>");
        process::exit(1); // Exit with an error code
    }
    let path = &args[1];
    get_deb_file(path);
}
