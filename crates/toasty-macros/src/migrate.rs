use proc_macro2::{Literal, TokenStream};
use quote::quote;
use std::path::PathBuf;
use toasty_core::migrate::{Config, HistoryFile};

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

    if !config_path.exists() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Toasty.toml not found at `{}`", config_path.display()),
        ));
    }

    let config = Config::from_path(&config_path)
        .map_err(|e| syn::Error::new(proc_macro2::Span::call_site(), e.to_string()))?;

    let migration_base = if config.migration.path.is_absolute() {
        config.migration.path
    } else {
        manifest_dir.join(config.migration.path)
    };

    let history_path = migration_base.join("history.toml");
    let migrations_dir = migration_base.join("migrations");

    let history = HistoryFile::load_or_default(&history_path)
        .map_err(|e| syn::Error::new(proc_macro2::Span::call_site(), e.to_string()))?;

    let entries = history.migrations().iter().map(|m| {
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
