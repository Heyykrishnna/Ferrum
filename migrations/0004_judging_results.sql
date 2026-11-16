CREATE TABLE submission_test_cases (
    organization_id uuid NOT NULL,
    submission_id uuid NOT NULL,
    case_index integer NOT NULL,
    input text NOT NULL,
    expected_output text NOT NULL,
    weight integer NOT NULL DEFAULT 1,
    hidden boolean NOT NULL DEFAULT false,
    comparison_mode text NOT NULL DEFAULT 'exact',
    PRIMARY KEY (organization_id, submission_id, case_index),
    FOREIGN KEY (organization_id, submission_id) REFERENCES submissions (organization_id, id)
);

CREATE TABLE submission_case_results (
    attempt_id uuid NOT NULL REFERENCES execution_results (attempt_id),
    case_index integer NOT NULL,
    verdict text NOT NULL,
    score integer NOT NULL DEFAULT 0,
    max_score integer NOT NULL DEFAULT 1,
    stdout bytea NOT NULL CHECK (octet_length(stdout) <= 65536),
    stderr bytea NOT NULL CHECK (octet_length(stderr) <= 65536),
    exit_code smallint CHECK (exit_code BETWEEN 0 AND 255),
    signal smallint CHECK (signal BETWEEN 0 AND 255),
    cpu_time_ms bigint CHECK (cpu_time_ms >= 0),
    wall_time_ms bigint CHECK (wall_time_ms >= 0),
    peak_memory_kib bigint CHECK (peak_memory_kib >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (attempt_id, case_index)
);

ALTER TABLE execution_results ADD COLUMN score integer;
ALTER TABLE execution_results ADD COLUMN max_score integer;
