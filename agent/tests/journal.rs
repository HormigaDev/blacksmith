//! `Journalctl` contra procesos reales.
//!
//! - Un `journalctl` falso verifica argumentos, lectura continua, fin, errores y limpieza.
//! - El test `#[ignore]` usa el journal de verdad: `cargo test -p agent -- --ignored`.

// Los helpers no son `#[test]`, así que `allow-unwrap-in-tests` no los cubre.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
    time::Duration,
};

use agent::systemd::{JournalError, Journalctl, LogStream, Scope, UnitName};

/// El comportamiento depende del último argumento (`--unit=<unidad>`).
const FAKE: &str = r#"#!/bin/sh
for unit; do :; done
entry() { echo "{\"__REALTIME_TIMESTAMP\":\"$1\",\"MESSAGE\":\"$2\",\"PRIORITY\":\"6\",\"_PID\":\"42\"}"; }
case "$unit" in
    --unit=eco.service) entry 1 "$*"; exec /bin/sleep 30 ;;
    --unit=continuo.service)
        entry 1 uno; entry 2 dos
        /bin/sleep 0.3; entry 3 tres
        exec /bin/sleep 30 ;;
    --unit=fin.service) entry 1 ultima; exit 0 ;;
    --unit=falla.service) echo "Failed to open journal: Permission denied" >&2; exit 1 ;;
    --unit=basura.service) echo "no es json"; entry 1 "tras la basura"; exec /bin/sleep 30 ;;
esac
exit 99
"#;

fn fake() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("journalctl-falso");
        fs::write(&path, FAKE).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    })
}

fn follow(unit: &str) -> LogStream {
    Journalctl::new(fake(), Scope::System)
        .follow(&[UnitName::new(unit).unwrap()], 100)
        .unwrap()
}

async fn next_message(stream: &mut LogStream) -> String {
    tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("sin entrada a tiempo")
        .expect("flujo terminado")
        .expect("error de journalctl")
        .message
}

/// El proceso ya no existe ni como zombi.
async fn assert_reaped(pid: u32) {
    for _ in 0..50 {
        if !Path::new(&format!("/proc/{pid}")).exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let state = fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
    panic!("journalctl {pid} sigue existiendo:\n{state}");
}

#[tokio::test]
async fn argumentos_y_entorno() {
    let mut stream = follow("eco");
    assert_eq!(
        next_message(&mut stream).await,
        "--no-pager --quiet --output=json --output-fields=MESSAGE,PRIORITY,_PID,SYSLOG_IDENTIFIER --follow --merge --lines=100 --unit=eco.service"
    );
}

/// Solo desde un instante (la salida del último despliegue, no la de los anteriores).
#[tokio::test]
async fn desde_un_instante() {
    let mut stream = Journalctl::new(fake(), Scope::System)
        .follow_since(&[UnitName::new("eco").unwrap()], 1_792_713_740, 2000)
        .unwrap();
    assert_eq!(
        next_message(&mut stream).await,
        "--no-pager --quiet --output=json --output-fields=MESSAGE,PRIORITY,_PID,SYSLOG_IDENTIFIER --follow --merge --lines=2000 --since=@1792713740 --unit=eco.service"
    );
}

/// Un despliegue se lee de dos unidades (clon y preparación): una opción `--unit` por cada una.
#[tokio::test]
async fn varias_unidades() {
    let units = [UnitName::new("a").unwrap(), UnitName::new("eco").unwrap()];
    let mut stream = Journalctl::new(fake(), Scope::System)
        .follow(&units, 10)
        .unwrap();
    assert!(
        next_message(&mut stream)
            .await
            .ends_with("--lines=10 --unit=a.service --unit=eco.service")
    );
}

#[tokio::test]
async fn lectura_continua() {
    let mut stream = follow("continuo");
    assert_eq!(next_message(&mut stream).await, "uno");
    assert_eq!(next_message(&mut stream).await, "dos");
    // Llega después, mientras `journalctl` sigue vivo.
    assert_eq!(next_message(&mut stream).await, "tres");
}

#[tokio::test]
async fn soltar_el_flujo_mata_y_recoge_el_proceso() {
    let mut stream = follow("continuo");
    next_message(&mut stream).await;
    let pid = stream.pid().unwrap();
    drop(stream);
    assert_reaped(pid).await;
}

#[tokio::test]
async fn detecta_el_cierre() {
    let mut stream = follow("fin");
    assert_eq!(next_message(&mut stream).await, "ultima");
    let pid = stream.pid().unwrap();
    match stream.next().await {
        Some(Err(JournalError::Exited {
            status: Some(0), ..
        })) => {}
        other => panic!("se esperaba el cierre: {other:?}"),
    }
    assert!(stream.next().await.is_none());
    assert_reaped(pid).await;
}

#[tokio::test]
async fn detecta_el_fallo_con_su_mensaje() {
    let mut stream = follow("falla");
    match stream.next().await {
        Some(Err(JournalError::Exited {
            status: Some(1),
            detail,
        })) => {
            assert_eq!(detail, "Failed to open journal: Permission denied");
        }
        other => panic!("se esperaba el fallo: {other:?}"),
    }
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn una_linea_invalida_no_corta_el_flujo() {
    let mut stream = follow("basura");
    assert_eq!(next_message(&mut stream).await, "tras la basura");
}

#[tokio::test]
async fn programa_inexistente() {
    // El script falso se escribe antes de cualquier `fork`: si este test lanzara un proceso
    // mientras otro lo escribe, el hijo heredaría el descriptor de escritura y ejecutar el
    // script fallaría con ETXTBSY.
    fake();
    let result = Journalctl::new("/no/existe/journalctl", Scope::System)
        .follow(&[UnitName::new("x").unwrap()], 10);
    assert!(matches!(result, Err(JournalError::Spawn(_))));
}

// --- journal real ---------------------------------------------------------------------------

/// Unidad de usuario temporal que escribe en el journal; se lee lo anterior y lo nuevo.
#[tokio::test]
#[ignore = "usa el journal y el systemd de usuario reales (crea y borra una unidad temporal)"]
async fn journal_real_de_usuario() {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap();
    let dir = Path::new(&runtime).join("systemd/user");
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("blacksmith-prueba-log.service");
    fs::write(
        &file,
        "[Service]\nExecStart=/bin/sh -c 'echo antes; sleep 1; echo despues; sleep 300'\n",
    )
    .unwrap();
    let systemctl = |args: &[&str]| {
        Command::new("/usr/bin/systemctl")
            .arg("--user")
            .args(args)
            .status()
            .unwrap();
    };
    systemctl(&["daemon-reload"]);
    systemctl(&["start", "blacksmith-prueba-log.service"]);
    // El journal tarda un poco en indexar la primera línea.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut stream = Journalctl::new("/usr/bin/journalctl", Scope::User)
        .follow(&[UnitName::new("blacksmith-prueba-log").unwrap()], 100)
        .unwrap();
    let mut messages = Vec::new();
    while !messages.iter().any(|message: &String| message == "despues") {
        let message = tokio::time::timeout(Duration::from_secs(10), next_message(&mut stream))
            .await
            .expect("no llegó «despues»");
        messages.push(message);
    }
    let pid = stream.pid().unwrap();
    drop(stream);

    systemctl(&["stop", "blacksmith-prueba-log.service"]);
    fs::remove_file(&file).unwrap();
    systemctl(&["daemon-reload"]);
    assert!(messages.contains(&"antes".to_owned()), "{messages:?}");
    assert_reaped(pid).await;
}
