// Los comandos de la app se declaran aquí: así cada uno necesita un permiso explícito en
// capabilities/ (`allow-<comando>`) en vez de quedar expuesto a cualquier ventana por defecto.
const COMMANDS: &[&str] = &[
    "ssh_save_key",
    "ssh_key_info",
    "ssh_forget_key",
    "ssh_forget_host",
    "tunnel_open",
    "tunnel_trust_host",
    "tunnel_close",
    "tunnel_status",
    "servers_list",
    "server_remove",
];

fn main() {
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("error al generar el contexto de Tauri: {error:#}");
        std::process::exit(1);
    }
}
