"""Offline tests for untrusted source collection; no papers or network required."""
import hashlib
import importlib.util
import io
from pathlib import Path
import tarfile
import unittest

spec = importlib.util.spec_from_file_location('collector', Path(__file__).parents[1] / 'collect-real-documents.py')
collector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collector)


class SourceArchiveTests(unittest.TestCase):
    def archive(self, names):
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode='w:gz') as archive:
            for name, kind in names:
                member = tarfile.TarInfo(name)
                member.type = kind
                if kind == tarfile.REGTYPE:
                    member.size = 3
                    archive.addfile(member, io.BytesIO(b'tex'))
                else:
                    member.linkname = '/tmp/outside'
                    archive.addfile(member)
        return stream.getvalue()

    def test_reads_pinned_members_without_extracting_other_files(self):
        digest = hashlib.sha256(b'tex').hexdigest()
        result = collector.source_files(self.archive([('main.tex', tarfile.REGTYPE), ('figure.pdf', tarfile.REGTYPE)]), {'main.tex': digest})
        self.assertEqual(result, {'main.tex': b'tex'})

    def test_rejects_path_escape_links_and_duplicate_members(self):
        digest = hashlib.sha256(b'tex').hexdigest()
        for entries in [
            [('../main.tex', tarfile.REGTYPE)],
            [('/main.tex', tarfile.REGTYPE)],
            [('main.tex', tarfile.SYMTYPE)],
            [('main.tex', tarfile.LNKTYPE)],
            [('main.tex', tarfile.REGTYPE), ('main.tex', tarfile.REGTYPE)],
        ]:
            with self.subTest(entries=entries), self.assertRaises(ValueError):
                collector.source_files(self.archive(entries), {'main.tex': digest})

    def test_rejects_missing_or_modified_sources(self):
        for expected in [{'missing.tex': '0' * 64}, {'main.tex': '0' * 64}]:
            with self.subTest(expected=expected), self.assertRaises(ValueError):
                collector.source_files(self.archive([('main.tex', tarfile.REGTYPE)]), expected)


if __name__ == '__main__':
    unittest.main()
