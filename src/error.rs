use thiserror::Error;

pub type Result<T> = std::result::Result<T, RestTimeError>;

#[derive(Error, Debug)]
pub enum RestTimeError {
    #[error("Configuration IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("D-Bus communication failure: {0}")]
    DBus(#[from] zbus::Error),

    #[error("GTK/GLib initialization error: {0}")]
    Glib(#[from] glib::Error),

    #[error("Audio pipeline initialization failure")]
    AudioInit,
}
