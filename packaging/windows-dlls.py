#!/usr/bin/env python3
"""Copia a un directorio las DLLs necesarias, resolviendo dependencias recursivamente."""
import os
import shutil
import subprocess
import sys


def imported_dlls(path):
    try:
        output = subprocess.run(
            ["llvm-objdump", "-p", path],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        return []

    names = []
    for line in output.splitlines():
        line = line.strip()
        if line.lower().startswith("dll name:"):
            names.append(line.split(":", 1)[1].strip())
    return names


def main():
    if len(sys.argv) != 4:
        sys.exit("uso: windows-dlls.py <exe> <dir-dlls-origen> <dir-destino>")

    exe, source_dir, target_dir = sys.argv[1:]
    available = {name.lower(): name for name in os.listdir(source_dir)}

    pending = [exe]
    seen = set()

    while pending:
        current = pending.pop()
        for name in imported_dlls(current):
            key = name.lower()
            if key in seen:
                continue
            seen.add(key)
            real_name = available.get(key)
            if real_name is None:
                continue  # DLL del sistema
            src = os.path.join(source_dir, real_name)
            dst = os.path.join(target_dir, real_name)
            if not os.path.exists(dst):
                shutil.copy2(src, dst)
            pending.append(src)

    print(f"{len(seen)} DLLs inspeccionadas")


if __name__ == "__main__":
    main()
