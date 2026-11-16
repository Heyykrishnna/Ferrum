pub struct Fixture {
    pub language: &'static str,
    pub echo: &'static str,
    pub failure: &'static str,
    pub wait: &'static str,
}

pub const FIXTURES: &[Fixture] = &[
    Fixture {
        language: "python",
        echo: "print(input())",
        failure: "raise SystemExit(7)",
        wait: "import time\ntime.sleep(15)",
    },
    Fixture {
        language: "cpp",
        echo: "#include <iostream>\n#include <string>\nint main() { std::string line; std::getline(std::cin, line); std::cout << line << '\\n'; }",
        failure: "int main() { return 7; }",
        wait: "#include <unistd.h>\nint main() { sleep(15); }",
    },
    Fixture {
        language: "javascript",
        echo: "process.stdout.write(require('node:fs').readFileSync(0));",
        failure: "process.exit(7);",
        wait: "setTimeout(() => {}, 15000);",
    },
    Fixture {
        language: "c",
        echo: "#include <stdio.h>\nint main(void) { char line[128]; if (!fgets(line, sizeof line, stdin)) return 1; return fputs(line, stdout) < 0; }",
        failure: "int main(void) { return 7; }",
        wait: "#include <unistd.h>\nint main(void) { sleep(15); }",
    },
    Fixture {
        language: "java",
        echo: "import java.io.*; public class Main { public static void main(String[] args) throws Exception { System.out.println(new BufferedReader(new InputStreamReader(System.in)).readLine()); } }",
        failure: "public class Main { public static void main(String[] args) { System.exit(7); } }",
        wait: "public class Main { public static void main(String[] args) throws Exception { Thread.sleep(15000); } }",
    },
    Fixture {
        language: "rust",
        echo: "use std::io; fn main() { let mut line = String::new(); io::stdin().read_line(&mut line).unwrap(); print!(\"{line}\"); }",
        failure: "fn main() { std::process::exit(7); }",
        wait: "fn main() { std::thread::sleep(std::time::Duration::from_secs(15)); }",
    },
];
