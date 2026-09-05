#!/usr/bin/env python3
"""Fetch pinned arXiv source archives without executing TeX or redistributing papers."""
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
LIMIT = 20 * 1024 * 1024


def checked_bytes(data, digest, label):
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f"{label}: SHA-256 mismatch")
    return data


def source_files(data, expected):
    """Read only pinned regular members; never extract archive paths to disk."""
    result = {}
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        members = archive.getmembers()
        if len(members) > 1000 or sum(m.size for m in members) > LIMIT:
            raise ValueError("archive exceeds collection bounds")
        for member in members:
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or member.issym() or member.islnk():
                raise ValueError("unsafe archive member")
            name = str(path)
            if name not in expected:
                continue
            if not member.isfile() or name in result:
                raise ValueError("source member must be a unique regular file")
            result[name] = checked_bytes(archive.extractfile(member).read(), expected[name], name)
    if result.keys() != expected.keys():
        raise ValueError("archive does not contain the pinned source inventory")
    return result


def main():
    cache = ROOT / '.artifacts/real-documents'
    cache.mkdir(parents=True, exist_ok=True)
    sources = json.loads((ROOT / 'fixtures/real-documents/sources.json').read_text())
    for source in sources:
        archive = cache / (source['id'] + '.tar')
        if archive.exists():
            data = archive.read_bytes()
        else:
            request = urllib.request.Request(source['sourceUrl'], headers={'User-Agent': 'Semath-source-evaluation/1.0'})
            with urllib.request.urlopen(request, timeout=60) as response:
                data = response.read(LIMIT + 1)
        if len(data) > LIMIT:
            raise ValueError('download exceeds collection bounds')
        checked_bytes(data, source['archiveSha256'], source['id'])
        expected = {item['path']: item['sha256'] for item in source['files']}
        files = source_files(data, expected)
        archive.write_bytes(data)
        for name, content in files.items():
            destination = cache / source['id'] / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(content)
        print(f"{source['id']}: verified {len(files)} source files ({source['version']})")


if __name__ == '__main__':
    main()
