//! Gestión de servicios `systemd` mediante `systemctl`.
//!
//! - [`UnitName`]: nombre de unidad `.service` validado. Es la única forma de nombrar una unidad,
//!   así que ningún texto del cliente llega a `systemctl` sin pasar por la validación.
//! - [`Systemctl`]: acciones de ciclo de vida (`start`, `stop`, `restart`) con resultado tipado
//!   ([`Outcome`]) o error clasificado ([`SystemctlError`]).
//!
//! El agente corre sin privilegios: para gestionar servicios del sistema necesita que polkit
//! autorice al usuario de servicio. Sin esa regla, toda acción termina en
//! [`SystemctlError::PermissionDenied`].

mod journal;
mod systemctl;
mod unit;

pub use journal::{JOURNALCTL_PATH, JournalError, Journalctl, LogStream};
pub use systemctl::{
    Action, DEFAULT_TIMEOUT, Operation, Outcome, SYSTEMCTL_PATH, Scope, Systemctl, SystemctlError,
    UnitResources, UnitStatus,
};
pub use unit::UnitName;
