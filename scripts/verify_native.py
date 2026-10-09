#!/usr/bin/env python3
"""Check both native ABIs and ensure the HAP contains no web runtime snapshot."""
import argparse
import struct
import zipfile
from pathlib import Path


def verify(path: Path) -> None:
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        for abi, machine in [('arm64-v8a', 183), ('x86_64', 62)]:
            name = f'libs/{abi}/libphotocraft.so'
            if name not in names:
                raise ValueError(f'missing {name}')
            data = archive.read(name)
            if data[:5] != b'\x7fELF\x02' or struct.unpack_from('<H', data, 18)[0] != machine:
                raise ValueError(f'incorrect ELF architecture: {name}')
        forbidden = [name for name in names if name.lower().endswith(('.wasm', '.html', '.js'))]
        if forbidden:
            raise ValueError(f'web assets remain: {forbidden}')
    print(f'Native HAP verified: {path}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('hap', type=Path)
    verify(parser.parse_args().hap)
