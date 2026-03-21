use proc_macro2::{Literal, TokenStream};
use quote::quote;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct ToastyConfig {
    #[serde(default)]
    migration: MigrationConfig,
}

#[derive(Deserialize)]
struct MigrationConfig {
    #[serde(default = "default_migration_path")]
    path: PathBuf,
}

fn default_migration_path() -> PathBuf {
    PathBuf::from("toasty")
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            path: default_migration_path(),
        }
    }
}

#[derive(Deserialize)]
struct HistoryFile {
    #[serde(default)]
    migrations: Vec<HistoryEntry>,
}

#[derive(Deserialize)]
struct HistoryEntry {
    id: u64,
    name: String,
}

pub(crate) fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let config_path_override: Option<syn::LitStr> = if input.is_empty() {
        None
    } else {
        Some(syn::parse2(input)?)
    };

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
        syn::Error::new(proc_macro2::Span::call_site(), "CARGO_MANIFEST_DIR not set")
    })?;
    let manifest_dir = PathBuf::from(manifest_dir);

    let config_path = match config_path_override {
        Some(lit) => manifest_dir.join(lit.value()),
        None => manifest_dir.join("Toasty.toml"),
    };

    let config_contents = std::fs::read_to_string(&config_path).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("failed to read {}: {}", config_path.display(), e),
        )
    })?;

    let config: ToastyConfig = toml::from_str(&config_contents).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("failed to parse {}: {}", config_path.display(), e),
        )
    })?;

    let migration_base = if config.migration.path.is_absolute() {
        config.migration.path
    } else {
        manifest_dir.join(config.migration.path)
    };

    let history_path = migration_base.join("history.toml");
    let migrations_dir = migration_base.join("migrations");

    let migrations = if history_path.exists() {
        let history_contents = std::fs::read_to_string(&history_path).map_err(|e| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("failed to read {}: {}", history_path.display(), e),
            )
        })?;
        let history: HistoryFile = toml::from_str(&history_contents).map_err(|e| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("failed to parse {}: {}", history_path.display(), e),
            )
        })?;
        history.migrations
    } else {
        Vec::new()
    };

    let entries = migrations.iter().map(|m| {
        let id = Literal::u64_suffixed(m.id);
        let name = &m.name;
        let sql_path = migrations_dir.join(&m.name);
        let sql_path_str = sql_path.to_string_lossy().into_owned();

        quote! {
            ::toasty::migrate::EmbeddedMigration {
                id: #id,
                name: #name,
                sql: include_str!(#sql_path_str),
            }
        }
    });

    Ok(quote! {
        {
            static MIGRATIONS: &[::toasty::migrate::EmbeddedMigration] = &[
                #(#entries),*
            ];
            ::toasty::migrate::Migrator::new(MIGRATIONS)
        }
    })
}
