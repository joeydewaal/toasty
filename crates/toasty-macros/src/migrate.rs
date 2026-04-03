use proc_macro2::TokenStream;
use quote::quote;
use syn::LitStr;
use toasty_core::schema::db::HistoryFile;

pub(crate) fn generate(input: TokenStream) -> syn::Result<TokenStream> {
    let rel_path = if input.is_empty() {
        "toasty".to_string()
    } else {
        let lit: LitStr = syn::parse2(input)?;
        lit.value()
    };

    // Resolve base directory from CARGO_MANIFEST_DIR
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let base = std::path::PathBuf::from(&manifest_dir).join(&rel_path);

    // Read and parse history.toml
    let history_path = base.join("history.toml");
    let history_content = std::fs::read_to_string(&history_path).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("failed to read {}: {e}", history_path.display()),
        )
    })?;

    let history: HistoryFile = toml::from_str(&history_content).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("failed to parse {}: {e}", history_path.display()),
        )
    })?;

    if !history.is_supported_version() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "unsupported history file version {} in {}; expected 1",
                history.version(),
                history_path.display()
            ),
        ));
    }

    let migrations_dir = base.join("migrations");
    let history_path_str = history_path.to_string_lossy().to_string();

    let mut entries = Vec::new();
    for m in history.migrations() {
        let id = m.id;
        let name = &m.name;

        let sql_path = migrations_dir.join(name);
        if !sql_path.exists() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("migration file not found: {}", sql_path.display()),
            ));
        }

        let sql_path_str = sql_path.to_string_lossy().to_string();

        entries.push(quote! {
            ::toasty::codegen_support::EmbeddedMigration {
                id: #id,
                name: #name,
                sql: include_str!(#sql_path_str),
            }
        });
    }

    Ok(quote! {
        {
            // include_str! on history.toml so Cargo tracks it for rebuild
            const _: &str = include_str!(#history_path_str);
            ::toasty::codegen_support::Migrator::new(&[
                #(#entries),*
            ])
        }
    })
}
