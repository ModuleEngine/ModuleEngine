"""Reject newly introduced documentation links to checkout-local artifacts."""

import argparse
from collections import Counter
import html
from html.parser import HTMLParser
import posixpath
import re
import subprocess
import sys
from urllib.parse import unquote


FENCE = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")
QUOTE = re.compile(r"^ {0,3}>[ \t]?")
LIST_MARKER = r" {0,3}(?:[-+*]|[0-9]{1,9}[.)])[ \t]{1,4}"
LIST_PREFIX = re.compile(rf"^(?:{LIST_MARKER})+")
INLINE_CODE = re.compile(r"(?<![\\`])(`+)(?!`)[\s\S]*?(?<!`)\1(?!`)")
ANGLE_DESTINATION = re.compile(r"<((?:\\.|[^<>\n])*?)>")
MARKDOWN_LINK = re.compile(
    rf"(?<!\\)\]\([ \t\n]*|^(?:{LIST_MARKER})* {{0,3}}\[[^\]\n]+\]:[ \t]*(?:\n[ \t]*)?",
    re.M,
)
HTML_TARGETS = {
    "a": {"href"}, "area": {"href"}, "link": {"href"}, "img": {"src"},
    "audio": {"src"}, "video": {"src", "poster"}, "source": {"src"},
    "iframe": {"src"}, "object": {"data"},
}
FILE_AUTOLINK = re.compile(r"<(file:[^<>\s]+)>", re.I)
DOC_TYPES = (".md", ".markdown", ".html", ".htm")


def blank(match):
    return re.sub(r"[^\n]", " ", match.group())


class HtmlLinks(HTMLParser):
    def __init__(self):
        super().__init__()
        self.targets = []

    def handle_starttag(self, tag, attrs):
        attributes = HTML_TARGETS.get(tag)
        if attributes:
            self.targets.extend((self.getpos()[0], value) for name, value in attrs
                                if name in attributes and value)


def markdown_destination(document, start):
    if document[start:start + 1] == "<":
        match = ANGLE_DESTINATION.match(document, start)
        return match[1] if match else None
    depth = 0
    end = start
    while end < len(document):
        char = document[end]
        if char.isspace() or (char == ")" and depth == 0):
            break
        if char == "\\" and document[end + 1:end + 2] in {"(", ")", "\\"}:
            end += 2
            continue
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
        end += 1
    if depth == 0 and end > start:
        return document[start:end]
    return None


def link_targets(document, markdown=True):
    """Yield line numbers and destinations, keeping code samples out of the scan."""
    document = re.sub(r"<!--.*?-->", blank, document, flags=re.S)
    if markdown:
        lines = []
        fence = ""
        fence_quote_level = 0
        fence_list_indent = 0
        for line in document.splitlines(keepends=True):
            quote_level = 0
            while (quote := QUOTE.match(line)) is not None:
                quote_level += 1
                line = line[quote.end():]
            indent = len(line) - len(line.lstrip(" \t"))
            if fence and (quote_level < fence_quote_level
                          or (line.strip() and indent < fence_list_indent)):
                fence = ""
            if fence:
                marker = FENCE.match(line[fence_list_indent:])
                lines.append(re.sub(r"[^\n]", " ", line))
                if (marker and quote_level == fence_quote_level
                        and marker[1].startswith(fence) and not marker[2].strip()):
                    fence = ""
                continue
            prefix = LIST_PREFIX.match(line)
            list_indent = prefix.end() if prefix else 0
            marker = FENCE.match(line[list_indent:])
            if marker:
                fence, fence_quote_level, fence_list_indent = marker[1], quote_level, list_indent
                line = re.sub(r"[^\n]", " ", line)
            lines.append(line)
        document = INLINE_CODE.sub(blank, "".join(lines))
        for link in MARKDOWN_LINK.finditer(document):
            dest = markdown_destination(document, link.end())
            if dest:
                yield document.count("\n", 0, link.end()) + 1, dest
    parser = HtmlLinks()
    parser.feed(document)
    parser.close()
    yield from parser.targets
    for link in FILE_AUTOLINK.finditer(document):
        yield document.count("\n", 0, link.start()) + 1, link[1]


def forbidden_target(doc, target):
    target = html.unescape(target).strip()
    path = unquote(re.split(r"[#?]", target, maxsplit=1)[0]).replace("\\", "/")
    if re.match(r"^(?:file:|[a-z]:|/|~/)", path, re.I):
        return "absolute local path", path
    if re.match(r"^[a-z][a-z0-9+.-]*:", path, re.I):
        return None
    destination = posixpath.normpath(posixpath.join(posixpath.dirname(doc), path))
    if destination.split("/", 1)[0].lower() in {"work", "outputs"}:
        return "checkout-local artifacts", destination
    return None


def violations(doc, contents):
    markdown = doc.lower().endswith((".md", ".markdown"))
    for line, target in sorted(link_targets(contents, markdown)):
        key = forbidden_target(doc, target)
        if key:
            yield key, line, target


def new_violations(doc, before, after):
    inherited = Counter(key for key, _, _ in violations(doc, before))
    for key, line, target in violations(doc, after):
        if inherited[key]:
            inherited[key] -= 1
        else:
            yield line, target, key[0]


def git(*args):
    return subprocess.check_output(["git", *args]).decode("utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="", help="Base SHA from the CI event")
    args = parser.parse_args()
    try:
        base = args.base if args.base.strip("0") else "HEAD^"
        base = git("rev-parse", "--verify", f"{base}^{{commit}}").strip()
        changed = git("diff", "--name-status", "--no-renames", "-z", base, "HEAD", "--", "docs/",
                      "AGENTS.md", "README.md", "README.ru.md",
                      ":(glob)crates/*/README.md", ":(glob)crates/*/README.ru.md",
                      ":(glob)examples/*/README.md", ":(glob)examples/*/README.ru.md")
        fields = changed.rstrip("\0").split("\0")
        failed = False
        for status, doc in zip(fields[::2], fields[1::2]):
            if status == "D" or not doc.lower().endswith(DOC_TYPES):
                continue
            before = "" if status == "A" else git("show", f"{base}:{doc}")
            after = git("show", f"HEAD:{doc}")
            for line, target, reason in new_violations(doc, before, after):
                print(f"{doc}:{line}: {reason}: {target}", file=sys.stderr)
                failed = True
        if not failed:
            print("No new documentation links to checkout-local paths.")
        return int(failed)
    except (subprocess.CalledProcessError, UnicodeError) as error:
        print(f"Cannot check documentation links: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
