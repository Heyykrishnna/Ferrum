import exec from "k6/execution";
import http from "k6/http";
import { check } from "k6";
import { Counter, Trend } from "k6/metrics";

const intakeDuration = new Trend("intake_duration", true);
const intakeByLanguage = new Trend("intake_duration_language", true);
const cancelDuration = new Trend("cancel_duration", true);
const intakeAccepted = new Counter("intake_accepted");
const intakeRejected = new Counter("intake_rejected");

const PROGRAMS = [
  { language: "python", weight: 35, source: "print(sum(range(100)))" },
  {
    language: "c",
    weight: 20,
    source: '#include <stdio.h>\nint main(){printf("%d\\n", 4950); return 0;}',
  },
  {
    language: "cpp",
    weight: 20,
    source: "#include <iostream>\nint main(){std::cout << 4950 << std::endl; return 0;}",
  },
  { language: "javascript", weight: 15, source: "console.log(4950);" },
  { language: "rust", weight: 10, source: 'fn main(){println!("{}", 4950);}' },
];

const MIX = PROGRAMS.flatMap((program) => Array.from({ length: program.weight }, () => program));

const rate = Number(__ENV.FERRUM_K6_RATE || 10);
const duration = __ENV.FERRUM_K6_DURATION || "15s";
const summaryPath = __ENV.FERRUM_K6_SUMMARY_PATH || "";

if (!Number.isInteger(rate) || rate < 1 || rate > 50) {
  throw new Error("FERRUM_K6_RATE must be an integer from 1 to 50 iterations per second");
}
if (!/^\d+(ms|s|m|h)$/.test(duration)) {
  throw new Error("FERRUM_K6_DURATION must look like 15s, 1m, or 5m");
}

export const options = {
  scenarios: {
    intake: {
      executor: "constant-arrival-rate",
      rate,
      timeUnit: "1s",
      duration,
      preAllocatedVUs: 6,
      maxVUs: 20,
    },
  },
  noConnectionReuse: true,
  summaryTrendStats: ["avg", "min", "med", "p(50)", "p(90)", "p(95)", "p(99)", "max", "count"],
  thresholds: {
    checks: ["rate>0.99"],
    dropped_iterations: ["count==0"],
    intake_rejected: ["count==0"],
  },
  tags: { benchmark: "ferrum-intake-regression" },
};

function pickProgram() {
  return MIX[exec.scenario.iterationInTest % MIX.length];
}

function idempotencyKey() {
  return `perf.${exec.vu.idInInstance}.${exec.scenario.iterationInInstance}.${Date.now()}`;
}

export function setup() {
  const base = __ENV.FERRUM_BASE_URL || "http://127.0.0.1:8081";
  const token = __ENV.FERRUM_DEV_TOKEN;
  if (!token) {
    throw new Error("FERRUM_DEV_TOKEN is required");
  }

  const live = http.get(`${base}/health/live`, { timeout: "5s", tags: { name: "setup" } });
  if (live.status !== 200) {
    throw new Error(`GET /health/live returned ${live.status} from ${base}`);
  }
  const ready = http.get(`${base}/health/ready`, { timeout: "5s", tags: { name: "setup" } });
  if (ready.status !== 200) {
    throw new Error(`GET /health/ready returned ${ready.status}: ${ready.body}`);
  }

  const languages = http.get(`${base}/v1/languages`, {
    headers: { Authorization: `Bearer ${token}` },
    timeout: "5s",
    tags: { name: "setup" },
  });
  if (languages.status !== 200) {
    throw new Error(`GET /v1/languages returned ${languages.status}`);
  }
  const advertised = new Set(languages.json().map((item) => item.language));
  const required = ["python", "c", "cpp", "javascript", "rust"];
  const missing = required.filter((language) => !advertised.has(language));
  if (missing.length > 0) {
    throw new Error(`API is missing language profiles: ${missing.join(", ")}`);
  }

  return { base, token };
}

export default function (data) {
  const program = pickProgram();
  const response = http.post(
    `${data.base}/v1/submissions`,
    JSON.stringify({ language: program.language, source: program.source }),
    {
      headers: {
        Authorization: `Bearer ${data.token}`,
        "Content-Type": "application/json",
        "Idempotency-Key": idempotencyKey(),
      },
      timeout: "10s",
      tags: { name: "intake", language: program.language },
    },
  );

  const location = response.headers.Location || response.headers.location;
  const accepted = check(response, {
    "intake status is 202": (result) => result.status === 202,
    "intake returns a location": () => Boolean(location),
  });

  if (!accepted) {
    intakeRejected.add(1, { status: String(response.status), language: program.language });
    return;
  }

  intakeDuration.add(response.timings.duration);
  intakeByLanguage.add(response.timings.duration, { language: program.language });
  intakeAccepted.add(1, { language: program.language });

  const cancel = http.post(`${data.base}${location}/cancel`, null, {
    headers: { Authorization: `Bearer ${data.token}` },
    timeout: "10s",
    tags: { name: "cancel", language: program.language },
  });
  cancelDuration.add(cancel.timings.duration, { language: program.language });
  const cancelled = check(cancel, {
    "cancel status is 200": (result) => result.status === 200,
    "cancel outcome is cancelled": (result) => result.json("outcome") === "cancelled",
  });
  if (!cancelled) {
    intakeRejected.add(1, { status: `cancel-${cancel.status}`, language: program.language });
  }
}

export function handleSummary(data) {
  const outputs = {};
  if (summaryPath) {
    outputs[summaryPath] = JSON.stringify(data, null, 2);
  }
  return outputs;
}

