import json
import subprocess
import os

compiler_path = os.path.join("..", "varg-compiler", "target", "release", "vargc.exe")
if not os.path.exists(compiler_path):
    compiler_path = os.path.join("..", "varg-compiler", "target", "debug", "vargc.exe")
    if not os.path.exists(compiler_path):
        print(f"Compiler not found! Tested {compiler_path}")
        exit(1)

print(f"Using compiler: {compiler_path}")

success = 0
failures = []

with open("varg_trainingsdaten.jsonl", "r", encoding="utf-8") as f:
    lines = f.readlines()

for idx, line in enumerate(lines):
    try:
        data = json.loads(line)
    except:
        continue
        
    code = data.get("output", "")
    if not code:
        continue
        
    # Write temp file
    temp_file = f"temp_validate.varg"
    with open(temp_file, "w", encoding="utf-8") as tf:
        tf.write(code)
        
    # Test compiler (emit-rs validates AST & Typechecking without full rustc build)
    result = subprocess.run([compiler_path, "emit-rs", temp_file], capture_output=True, text=True)
    
    if result.returncode == 0:
        success += 1
    else:
        failures.append((idx + 1, result.stderr))

if os.path.exists("temp_validate.varg"):
    os.remove("temp_validate.varg")

print(f"Total tested: {len(lines)}")
print(f"Success: {success}")
print(f"Failed: {len(failures)}")

if failures:
    print("\nFirst 3 failures:")
    for f in failures[:3]:
        print(f"Line {f[0]}: {f[1][:200]}...")
