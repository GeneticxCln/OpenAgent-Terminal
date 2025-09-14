//! Structured command completion specifications (embedded)
//! Local-first: static specs for common commands. No external fetching.

use once_cell::sync::Lazy;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FlagSpec {
    pub flag: &'static str,
    pub desc: &'static str,
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub name: &'static str,
    pub flags: &'static [FlagSpec],
    pub subcommands: &'static [&'static str],
}

static GIT_FLAGS: &[FlagSpec] = &[
    FlagSpec { flag: "--help", desc: "Show help for git or a subcommand" },
    FlagSpec { flag: "-C", desc: "Run as if git was started in <path>" },
    FlagSpec { flag: "-c", desc: "Pass a configuration parameter" },
    FlagSpec { flag: "--version", desc: "Show version information" },
    FlagSpec { flag: "--no-pager", desc: "Do not pipe git output into a pager" },
];

static LS_FLAGS: &[FlagSpec] = &[
    FlagSpec { flag: "-l", desc: "Use a long listing format" },
    FlagSpec { flag: "-a", desc: "Do not ignore entries starting with ." },
    FlagSpec { flag: "-h", desc: "With -l, print sizes in human readable format" },
    FlagSpec { flag: "-R", desc: "List subdirectories recursively" },
];

static CARGO_FLAGS: &[FlagSpec] = &[
    FlagSpec { flag: "--help", desc: "Print this message or the help of the given subcommand(s)" },
    FlagSpec { flag: "-v", desc: "Use verbose output (-vv very verbose)" },
    FlagSpec { flag: "-q", desc: "No output printed to stdout" },
];

static DOCKER_FLAGS: &[FlagSpec] = &[
    FlagSpec { flag: "--help", desc: "Help for docker or subcommand" },
    FlagSpec { flag: "-q", desc: "Only display IDs" },
    FlagSpec { flag: "--rm", desc: "Automatically remove container when it exits" },
];

static KUBECTL_FLAGS: &[FlagSpec] = &[
    FlagSpec { flag: "-n", desc: "Namespace scope" },
    FlagSpec { flag: "--namespace", desc: "Namespace scope" },
    FlagSpec { flag: "-o", desc: "Output format" },
    FlagSpec { flag: "-A", desc: "All namespaces" },
];

static SPECS: Lazy<HashMap<&'static str, CommandSpec>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert(
        "git",
        CommandSpec {
            name: "git",
            flags: GIT_FLAGS,
            subcommands: &[
                "add", "branch", "checkout", "clone", "commit", "diff", "fetch", "init", "log", "merge", "pull", "push", "rebase", "remote", "reset", "restore", "status", "switch", "tag",
            ],
        },
    );
    m.insert(
        "ls",
        CommandSpec { name: "ls", flags: LS_FLAGS, subcommands: &[] },
    );
    m.insert(
        "cargo",
        CommandSpec {
            name: "cargo",
            flags: CARGO_FLAGS,
            subcommands: &[
                "add", "bench", "build", "check", "clean", "clippy", "doc", "fix", "fmt", "init", "install", "login", "metadata", "new", "package", "publish", "run", "search", "test", "update", "vendor", "verify-project",
            ],
        },
    );
    m.insert(
        "docker",
        CommandSpec {
            name: "docker",
            flags: DOCKER_FLAGS,
            subcommands: &[
                "build", "compose", "cp", "create", "exec", "images", "info", "inspect", "kill", "logs", "network", "ps", "pull", "push", "restart", "rm", "rmi", "run", "start", "stats", "stop", "system", "volume",
            ],
        },
    );
    m.insert(
        "kubectl",
        CommandSpec {
            name: "kubectl",
            flags: KUBECTL_FLAGS,
            subcommands: &[
                "apply", "api-resources", "config", "cordon", "create", "delete", "describe", "drain", "edit", "exec", "explain", "get", "label", "logs", "patch", "port-forward", "rollout", "top",
            ],
        },
    );
    m
});

pub fn get_spec_for(cmd: &str) -> Option<&'static CommandSpec> {
    SPECS.get(cmd).or_else(|| {
        // Try common aliases
        let alias = match cmd {
            "k" => Some("kubectl"),
            _ => None,
        }?;
        SPECS.get(alias)
    })
}