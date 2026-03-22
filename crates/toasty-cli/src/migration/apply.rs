use super::{HistoryFile, SnapshotFile};
use crate::Config;
use anyhow::Result;
use clap::Parser;
use console::style;
use std::collections::HashSet;
use toasty::Db;
use toasty::schema::db::{RenameHints, Schema, SchemaDiff};

#[derive(Parser, Debug)]
pub struct ApplyCommand {}

impl ApplyCommand {
    pub(crate) async fn run(self, db: &Db, config: &Config) -> Result<()> {
        println!();
        println!("  {}", style("Apply Migrations").cyan().bold().underlined());
        println!();
        println!(
            "  {}",
            style(format!(
                "Connected to {}",
                crate::utility::redact_url_password(&db.driver().url())
            ))
            .dim()
        );
        println!();

        apply_migrations(db, config).await
    }
}

pub(crate) async fn apply_migrations(db: &Db, config: &Config) -> Result<()> {
    let history_path = config.migration.get_history_file_path();

    // Load migration history
    let history = HistoryFile::load_or_default(&history_path)?;

    if history.migrations().is_empty() {
        println!(
            "  {}",
            style("No migrations found in history file.")
                .magenta()
                .dim()
        );
        println!();
        return Ok(());
    }

    // Get a connection to check which migrations have been applied
    let mut conn = db.driver().connect().await?;

    // Get list of already applied migrations
    let applied_migrations = conn.applied_migrations().await?;
    let applied_ids: HashSet<u64> = applied_migrations.iter().map(|m| m.id()).collect();

    // Find migrations that haven't been applied yet
    let pending_indices: Vec<usize> = history
        .migrations()
        .iter()
        .enumerate()
        .filter(|(_, m)| !applied_ids.contains(&m.id))
        .map(|(i, _)| i)
        .collect();

    if pending_indices.is_empty() {
        println!(
            "  {}",
            style("All migrations are already applied. Database is up to date.")
                .green()
                .dim()
        );
        println!();
        return Ok(());
    }

    let pending_count = pending_indices.len();
    println!(
        "  {} Found {} pending migration(s) to apply",
        style("→").cyan(),
        pending_count
    );
    println!();

    // Apply each pending migration
    for idx in &pending_indices {
        let migration_entry = &history.migrations()[*idx];

        // Load the snapshot for this migration
        let snapshot_path = config
            .migration
            .get_snapshots_dir()
            .join(&migration_entry.snapshot_name);
        let snapshot = SnapshotFile::load(&snapshot_path)?;

        // Load the previous snapshot's schema (or start from empty)
        let prev_schema = if *idx == 0 {
            Schema::default()
        } else {
            let prev_entry = &history.migrations()[idx - 1];
            let prev_path = config
                .migration
                .get_snapshots_dir()
                .join(&prev_entry.snapshot_name);
            SnapshotFile::load(&prev_path)?.schema
        };

        // Reconstruct the diff using stored rename hints and generate SQL for this driver
        let hints = snapshot
            .rename_hints
            .map(|h| h.into_rename_hints())
            .unwrap_or_else(RenameHints::new);
        let diff = SchemaDiff::from(&prev_schema, &snapshot.schema, &hints);
        let migration = db.driver().generate_migration(&diff);

        println!(
            "  {} Applying migration: {}",
            style("→").cyan(),
            style(&migration_entry.snapshot_name).bold()
        );

        // Apply the migration
        conn.apply_migration(
            migration_entry.id,
            migration_entry.snapshot_name.clone(),
            &migration,
        )
        .await?;

        println!(
            "  {} {}",
            style("✓").green().bold(),
            style(format!("Applied: {}", migration_entry.snapshot_name)).dim()
        );
    }

    println!();
    println!(
        "  {}",
        style(format!(
            "Successfully applied {} migration(s)",
            pending_count
        ))
        .green()
        .bold()
    );
    println!();

    Ok(())
}
