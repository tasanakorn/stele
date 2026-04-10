mod client;
mod commands;
mod config;
mod mcp_proxy;
mod output;

use clap::{Parser, Subcommand};
use client::SteleClient;
use config::{load_config, resolve_connection, CliArgs};

#[derive(Parser)]
#[command(
    name = "stele",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ")"),
    about = "CLI client for the Stele shared memory server"
)]
struct Cli {
    #[arg(long, env = "STELE_PROFILE", help = "Use named connection profile")]
    profile: Option<String>,

    #[arg(long, env = "STELE_URL", help = "Override server URL")]
    server_url: Option<String>,

    #[arg(long, env = "STELE_AUTH_KEY", help = "Override auth key")]
    auth_key: Option<String>,

    #[arg(long, help = "Output raw JSON instead of formatted text")]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Create a memory")]
    Store {
        #[arg(long, required = true)]
        title: String,
        #[arg(long, required = true)]
        content: String,
        #[arg(long, required = true)]
        scope: String,
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
        #[arg(long, name = "type")]
        memory_type: Option<String>,
        #[arg(long)]
        author: Option<String>,
    },

    #[command(about = "Search memories")]
    Recall {
        query: Option<String>,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
        #[arg(long)]
        match_all_tags: bool,
        #[arg(long)]
        limit: Option<u32>,
    },

    #[command(about = "Get memory by ID")]
    Get { id: String },

    #[command(about = "Update memory")]
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
        #[arg(long, name = "type")]
        memory_type: Option<String>,
    },

    #[command(about = "Delete memory")]
    Forget { id: String },

    #[command(about = "List scopes")]
    Scopes {
        #[arg(long)]
        prefix: Option<String>,
    },

    #[command(about = "List tags")]
    Tags {
        #[arg(long)]
        scope: Option<String>,
    },

    #[command(about = "Show server stats")]
    Stats,

    #[command(about = "Health check")]
    Status,

    #[command(about = "Knowledge graph operations")]
    Graph {
        #[command(subcommand)]
        command: GraphCommands,
    },

    #[command(about = "MCP stdio proxy")]
    Mcp,

    #[command(about = "Config management")]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum GraphCommands {
    #[command(about = "Read full graph")]
    Read {
        #[arg(long, required = true)]
        scope: String,
    },

    #[command(about = "Search graph nodes")]
    Search {
        query: String,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },

    #[command(about = "Open specific nodes")]
    Open {
        names: String,
        #[arg(long)]
        scope: Option<String>,
    },

    #[command(about = "Entity operations")]
    Entities {
        #[command(subcommand)]
        command: EntitiesCommands,
    },

    #[command(about = "Observation operations")]
    Observations {
        #[command(subcommand)]
        command: ObservationsCommands,
    },

    #[command(about = "Relation operations")]
    Relations {
        #[command(subcommand)]
        command: RelationsCommands,
    },
}

#[derive(Subcommand)]
enum EntitiesCommands {
    #[command(about = "Create entity")]
    Create {
        #[arg(long, required = true)]
        name: String,
        #[arg(long, name = "type", required = true)]
        entity_type: String,
        #[arg(long, required = true)]
        scope: String,
        #[arg(long, value_delimiter = ',')]
        observations: Option<Vec<String>>,
    },

    #[command(about = "Get entity")]
    Get {
        name: String,
        #[arg(long, required = true)]
        scope: String,
    },

    #[command(about = "Delete entity")]
    Delete {
        name: String,
        #[arg(long, required = true)]
        scope: String,
    },
}

#[derive(Subcommand)]
enum ObservationsCommands {
    #[command(about = "Add observations to entity")]
    Add {
        entity_name: String,
        #[arg(long, required = true)]
        scope: String,
        #[arg(long, required = true, value_delimiter = ',')]
        observations: Vec<String>,
    },

    #[command(about = "Delete observations from entity")]
    Delete {
        entity_name: String,
        #[arg(long, required = true)]
        scope: String,
        #[arg(long, required = true, value_delimiter = ',')]
        observations: Vec<String>,
    },
}

#[derive(Subcommand)]
enum RelationsCommands {
    #[command(about = "Create relation")]
    Create {
        #[arg(long, required = true)]
        from: String,
        #[arg(long, required = true)]
        to: String,
        #[arg(long, name = "type", required = true)]
        relation_type: String,
        #[arg(long, required = true)]
        scope: String,
    },

    #[command(about = "Delete relation")]
    Delete {
        #[arg(long, required = true)]
        from: String,
        #[arg(long, required = true)]
        to: String,
        #[arg(long, name = "type", required = true)]
        relation_type: String,
        #[arg(long, required = true)]
        scope: String,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    #[command(about = "Create default config")]
    Init,
    #[command(about = "Show current config")]
    Show,
    #[command(about = "Print config file path")]
    Path,
    #[command(about = "Add or update a connection profile")]
    Set {
        #[arg(help = "Profile name")]
        name: String,
        #[arg(long, required = true, help = "Server URL")]
        url: String,
        #[arg(long, help = "Auth key")]
        key: Option<String>,
        #[arg(long, help = "Set as default profile")]
        default: bool,
    },
    #[command(about = "Remove a connection profile")]
    Remove {
        #[arg(help = "Profile name")]
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let cli_args = CliArgs {
        profile: cli.profile,
        server_url: cli.server_url,
        auth_key: cli.auth_key,
    };

    // Config commands don't need a server connection
    if let Commands::Config { command } = &cli.command {
        match command {
            ConfigCommands::Init => commands::config_cmd::handle_config_init(),
            ConfigCommands::Show => commands::config_cmd::handle_config_show(),
            ConfigCommands::Path => commands::config_cmd::handle_config_path(),
            ConfigCommands::Set {
                name,
                url,
                key,
                default,
            } => commands::config_cmd::handle_config_set(name, url, key.clone(), *default),
            ConfigCommands::Remove { name } => commands::config_cmd::handle_config_remove(name),
        }
        return;
    }

    if let Commands::Mcp = &cli.command {
        let config = load_config();
        let default_profile = cli_args
            .profile
            .clone()
            .or_else(|| std::env::var("STELE_PROFILE").ok())
            .unwrap_or_else(|| config.default_profile.clone());
        mcp_proxy::run(config, default_profile);
        return;
    }

    let (url, key) = resolve_connection(&cli_args);
    let client = SteleClient::new(url, key);
    let json = cli.json;

    match cli.command {
        Commands::Store {
            title,
            content,
            scope,
            tags,
            memory_type,
            author,
        } => {
            commands::memory::handle_store(
                &client,
                &title,
                &content,
                &scope,
                tags,
                memory_type,
                author,
                json,
            );
        }

        Commands::Recall {
            query,
            scope,
            tags,
            match_all_tags,
            limit,
        } => {
            commands::memory::handle_recall(
                &client,
                query.as_deref(),
                scope.as_deref(),
                tags,
                match_all_tags,
                limit,
                json,
            );
        }

        Commands::Get { id } => {
            commands::memory::handle_get(&client, &id, json);
        }

        Commands::Update {
            id,
            title,
            content,
            scope,
            tags,
            memory_type,
        } => {
            commands::memory::handle_update(
                &client,
                &id,
                title,
                content,
                scope,
                tags,
                memory_type,
                json,
            );
        }

        Commands::Forget { id } => {
            commands::memory::handle_forget(&client, &id, json);
        }

        Commands::Scopes { prefix } => {
            commands::info::handle_scopes(&client, prefix.as_deref(), json);
        }

        Commands::Tags { scope } => {
            commands::info::handle_tags(&client, scope.as_deref(), json);
        }

        Commands::Stats => {
            commands::info::handle_stats(&client, json);
        }

        Commands::Status => {
            commands::info::handle_status(&client, json);
        }

        Commands::Graph { command } => match command {
            GraphCommands::Read { scope } => {
                commands::graph::handle_graph_read(&client, &scope, json);
            }
            GraphCommands::Search {
                query,
                scope,
                limit,
            } => {
                commands::graph::handle_graph_search(
                    &client,
                    &query,
                    scope.as_deref(),
                    limit,
                    json,
                );
            }
            GraphCommands::Open { names, scope } => {
                commands::graph::handle_graph_open(&client, &names, scope.as_deref(), json);
            }
            GraphCommands::Entities { command } => match command {
                EntitiesCommands::Create {
                    name,
                    entity_type,
                    scope,
                    observations,
                } => {
                    commands::graph::handle_entities_create(
                        &client,
                        &name,
                        &entity_type,
                        &scope,
                        observations,
                        json,
                    );
                }
                EntitiesCommands::Get { name, scope } => {
                    commands::graph::handle_entities_get(&client, &name, &scope, json);
                }
                EntitiesCommands::Delete { name, scope } => {
                    commands::graph::handle_entities_delete(&client, &name, &scope, json);
                }
            },
            GraphCommands::Observations { command } => match command {
                ObservationsCommands::Add {
                    entity_name,
                    scope,
                    observations,
                } => {
                    commands::graph::handle_observations_add(
                        &client,
                        &entity_name,
                        &scope,
                        observations,
                        json,
                    );
                }
                ObservationsCommands::Delete {
                    entity_name,
                    scope,
                    observations,
                } => {
                    commands::graph::handle_observations_delete(
                        &client,
                        &entity_name,
                        &scope,
                        observations,
                        json,
                    );
                }
            },
            GraphCommands::Relations { command } => match command {
                RelationsCommands::Create {
                    from,
                    to,
                    relation_type,
                    scope,
                } => {
                    commands::graph::handle_relations_create(
                        &client,
                        &from,
                        &to,
                        &relation_type,
                        &scope,
                        json,
                    );
                }
                RelationsCommands::Delete {
                    from,
                    to,
                    relation_type,
                    scope,
                } => {
                    commands::graph::handle_relations_delete(
                        &client,
                        &from,
                        &to,
                        &relation_type,
                        &scope,
                        json,
                    );
                }
            },
        },

        Commands::Config { .. } | Commands::Mcp => unreachable!(),
    }
}
