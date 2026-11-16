ALTER TABLE execution_results DROP CONSTRAINT execution_results_outcome_check;

ALTER TABLE execution_results ADD CONSTRAINT execution_results_outcome_check
    CHECK (outcome IN (
        'success', 'compilation_error', 'runtime_error', 'cpu_time_limit', 'wall_time_limit',
        'memory_limit', 'output_limit_stdout', 'output_limit_stderr', 'cancelled'
    ));

CREATE TABLE compilation_results (
    attempt_id uuid PRIMARY KEY REFERENCES execution_results (attempt_id),
    outcome text NOT NULL CHECK (outcome IN (
        'success', 'compilation_error', 'cpu_time_limit', 'wall_time_limit',
        'memory_limit', 'output_limit_stdout', 'output_limit_stderr', 'cancelled'
    )),
    stdout bytea NOT NULL CHECK (octet_length(stdout) <= 65536),
    stderr bytea NOT NULL CHECK (octet_length(stderr) <= 65536),
    exit_code smallint CHECK (exit_code BETWEEN 0 AND 255),
    signal smallint CHECK (signal BETWEEN 0 AND 255),
    cpu_time_ms bigint CHECK (cpu_time_ms >= 0),
    wall_time_ms bigint CHECK (wall_time_ms >= 0),
    peak_memory_kib bigint CHECK (peak_memory_kib >= 0)
);
