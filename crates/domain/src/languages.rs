use crate::EffectiveLimits;

pub struct CommandPlan {
    pub program: &'static str,
    pub args: &'static [&'static str],
}

pub struct Toolchain {
    pub program: &'static str,
    pub version_args: &'static [&'static str],
    pub version_prefix: &'static str,
}

pub struct CompilationPlan {
    pub command: CommandPlan,
    pub artifact_file: &'static str,
    pub limits: EffectiveLimits,
}

pub struct LanguageProfile {
    pub id: u16,
    pub language: &'static str,
    pub revision: &'static str,
    pub source_file: &'static str,
    pub toolchain: Toolchain,
    pub compile: Option<CompilationPlan>,
    pub run: CommandPlan,
    pub run_limits: EffectiveLimits,
}

const PYTHON_LIMITS: EffectiveLimits = EffectiveLimits {
    cpu_time_ms: 2_000,
    wall_time_ms: 6_000,
    memory_kib: 131_072,
    stack_kib: 8_192,
    processes: 1,
    open_files: 64,
    file_bytes: 65_536,
    workspace_kib: 4_096,
    workspace_inodes: 128,
    stdout_bytes: 65_536,
    stderr_bytes: 65_536,
};

const COMPILATION_LIMITS: EffectiveLimits = EffectiveLimits {
    cpu_time_ms: 8_000,
    wall_time_ms: 12_000,
    memory_kib: 524_288,
    processes: 64,
    open_files: 128,
    file_bytes: 8_388_608,
    workspace_kib: 32_768,
    workspace_inodes: 256,
    ..PYTHON_LIMITS
};

const MANAGED_LIMITS: EffectiveLimits = EffectiveLimits {
    cpu_time_ms: 3_000,
    memory_kib: 262_144,
    processes: 64,
    ..PYTHON_LIMITS
};

const NATIVE_LIMITS: EffectiveLimits = EffectiveLimits {
    workspace_kib: COMPILATION_LIMITS.workspace_kib,
    workspace_inodes: COMPILATION_LIMITS.workspace_inodes,
    ..PYTHON_LIMITS
};

pub const LANGUAGES: &[LanguageProfile] = &[
    LanguageProfile {
        id: 1,
        language: "python",
        revision: "python3.13-v1",
        source_file: "main.py",
        toolchain: Toolchain {
            program: "/usr/bin/python3",
            version_args: &["--version"],
            version_prefix: "Python 3.13.",
        },
        compile: None,
        run: CommandPlan {
            program: "/usr/bin/python3",
            args: &["-I", "-B", "/box/main.py"],
        },
        run_limits: PYTHON_LIMITS,
    },
    LanguageProfile {
        id: 2,
        language: "cpp",
        revision: "gcc14-cpp20-v1",
        source_file: "main.cpp",
        toolchain: Toolchain {
            program: "/usr/bin/g++-14",
            version_args: &["-dumpfullversion"],
            version_prefix: "14.2.",
        },
        compile: Some(CompilationPlan {
            command: CommandPlan {
                program: "/usr/bin/g++-14",
                args: &["-std=c++20", "-O0", "-o", "/box/program", "/box/main.cpp"],
            },
            artifact_file: "program",
            limits: COMPILATION_LIMITS,
        }),
        run: CommandPlan {
            program: "/box/program",
            args: &[],
        },
        run_limits: NATIVE_LIMITS,
    },
    LanguageProfile {
        id: 3,
        language: "javascript",
        revision: "node20.19-v1",
        source_file: "main.cjs",
        toolchain: Toolchain {
            program: "/usr/bin/node",
            version_args: &["--version"],
            version_prefix: "v20.19.",
        },
        compile: None,
        run: CommandPlan {
            program: "/usr/bin/node",
            args: &[
                "--max-old-space-size=96",
                "--v8-pool-size=2",
                "/box/main.cjs",
            ],
        },
        run_limits: MANAGED_LIMITS,
    },
    LanguageProfile {
        id: 4,
        language: "c",
        revision: "gcc14-c17-v1",
        source_file: "main.c",
        toolchain: Toolchain {
            program: "/usr/bin/gcc-14",
            version_args: &["-dumpfullversion"],
            version_prefix: "14.2.",
        },
        compile: Some(CompilationPlan {
            command: CommandPlan {
                program: "/usr/bin/gcc-14",
                args: &["-std=c17", "-O0", "-o", "/box/program", "/box/main.c"],
            },
            artifact_file: "program",
            limits: COMPILATION_LIMITS,
        }),
        run: CommandPlan {
            program: "/box/program",
            args: &[],
        },
        run_limits: NATIVE_LIMITS,
    },
    LanguageProfile {
        id: 5,
        language: "java",
        revision: "openjdk21-v1",
        source_file: "Main.java",
        toolchain: Toolchain {
            program: "/usr/lib/jvm/java-21-openjdk-amd64/bin/javac",
            version_args: &["-version"],
            version_prefix: "javac 21.",
        },
        compile: Some(CompilationPlan {
            command: CommandPlan {
                program: "/usr/lib/jvm/java-21-openjdk-amd64/bin/javac",
                args: &[
                    "-J-XX:ActiveProcessorCount=1",
                    "-J-XX:+UseSerialGC",
                    "-J-Xmx192m",
                    "-J-Djava.io.tmpdir=/box",
                    "-J-XX:-UsePerfData",
                    "-proc:none",
                    "-encoding",
                    "UTF-8",
                    "-classpath",
                    "/box",
                    "-d",
                    "/box",
                    "/box/Main.java",
                ],
            },
            artifact_file: "Main.class",
            limits: COMPILATION_LIMITS,
        }),
        run: CommandPlan {
            program: "/usr/lib/jvm/java-21-openjdk-amd64/bin/java",
            args: &[
                "-XX:ActiveProcessorCount=1",
                "-XX:+UseSerialGC",
                "-Xmx96m",
                "-Djava.io.tmpdir=/box",
                "-XX:-UsePerfData",
                "-cp",
                "/box",
                "Main",
            ],
        },
        run_limits: EffectiveLimits {
            workspace_kib: COMPILATION_LIMITS.workspace_kib,
            workspace_inodes: COMPILATION_LIMITS.workspace_inodes,
            ..MANAGED_LIMITS
        },
    },
    LanguageProfile {
        id: 6,
        language: "rust",
        revision: "rust1.85-v1",
        source_file: "main.rs",
        toolchain: Toolchain {
            program: "/usr/bin/rustc",
            version_args: &["--version"],
            version_prefix: "rustc 1.85.",
        },
        compile: Some(CompilationPlan {
            command: CommandPlan {
                program: "/usr/bin/rustc",
                args: &[
                    "--edition=2024",
                    "-C",
                    "opt-level=0",
                    "-C",
                    "codegen-units=1",
                    "-C",
                    "linker=/usr/bin/gcc-14",
                    "-o",
                    "/box/program",
                    "/box/main.rs",
                ],
            },
            artifact_file: "program",
            limits: EffectiveLimits {
                memory_kib: 786_432,
                ..COMPILATION_LIMITS
            },
        }),
        run: CommandPlan {
            program: "/box/program",
            args: &[],
        },
        run_limits: NATIVE_LIMITS,
    },
];

pub fn resolve_language(language: &str) -> Option<&'static LanguageProfile> {
    LANGUAGES
        .iter()
        .find(|profile| profile.language == language)
}

pub fn resolve_revision(revision: &str) -> Option<&'static LanguageProfile> {
    LANGUAGES
        .iter()
        .find(|profile| profile.revision == revision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_unique_ids_and_trusted_stage_plans() {
        for (index, profile) in LANGUAGES.iter().enumerate() {
            assert!(LANGUAGES[..index].iter().all(|previous| {
                previous.id != profile.id
                    && previous.language != profile.language
                    && previous.revision != profile.revision
            }));
            assert!(
                profile
                    .source_file
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.')
            );
            assert!(profile.toolchain.program.starts_with("/usr/"));
            assert!(
                profile.run.program.starts_with("/usr/") || profile.run.program == "/box/program"
            );
            if let Some(plan) = &profile.compile {
                assert_eq!(plan.command.program, profile.toolchain.program);
                assert!(
                    plan.artifact_file
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.')
                );
            }
        }
    }
}
