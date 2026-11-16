import type { Language } from '../shared/contracts';

export const languages = {
  python: {
    label: 'Python 3',
    file: 'main.py',
    source:
      'def greet(name: str) -> str:\n    return f"Hello, {name}!"\n\n\ndef main() -> None:\n    name = input().strip()\n    print(greet(name))\n\n\nif __name__ == "__main__":\n    main()\n',
  },
  cpp: {
    label: 'C++',
    file: 'main.cpp',
    source:
      '#include <iostream>\n#include <string>\n\nint main() {\n    std::string name;\n    std::getline(std::cin, name);\n    std::cout << "Hello, " << name << "!\\n";\n}\n',
  },
  javascript: {
    label: 'JavaScript',
    file: 'main.cjs',
    source:
      'const fs = require("node:fs");\n\nconst name = fs.readFileSync(0, "utf8").trim();\nconsole.log(`Hello, ${name}!`);\n',
  },
  c: {
    label: 'C',
    file: 'main.c',
    source:
      '#include <stdio.h>\n#include <string.h>\n\nint main(void) {\n    char name[128] = "";\n    fgets(name, sizeof name, stdin);\n    name[strcspn(name, "\\r\\n")] = \'\\0\';\n    printf("Hello, %s!\\n", name);\n    return 0;\n}\n',
  },
  java: {
    label: 'Java',
    file: 'Main.java',
    source:
      'import java.util.Scanner;\n\npublic class Main {\n    public static void main(String[] args) {\n        Scanner input = new Scanner(System.in);\n        System.out.println("Hello, " + input.nextLine() + "!");\n    }\n}\n',
  },
  rust: {
    label: 'Rust',
    file: 'main.rs',
    source:
      'use std::io;\n\nfn main() {\n    let mut name = String::new();\n    io::stdin().read_line(&mut name).unwrap();\n    println!("Hello, {}!", name.trim());\n}\n',
  },
} satisfies Record<Language, { label: string; file: string; source: string }>;

export const languageChoices = Object.keys(languages) as Language[];
