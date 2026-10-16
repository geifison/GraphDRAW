/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// gd-cdr-bridge: reads a .cdr/.cmx file through libcdr and writes every
// librevenge drawing callback as one JSON object per line (JSON Lines) on
// stdout. GraphDraw runs it as a separate process so that a crash inside the
// parser never takes the application down (GD-CDR-06).
//
// Usage:
//   gd-cdr-bridge -            read the file from stdin
//   gd-cdr-bridge <path>       read the file from disk
//   gd-cdr-bridge --version
//
// Exit codes: 0 ok, 1 usage/IO error, 2 unsupported or encrypted, 3 parse error.
//
// Line format:
//   {"op":"<callback>","p":{<properties>}}            calls with a property list
//   {"op":"insertText","text":"..."}                  text runs
// Property values are [string, unit, number] where unit is one of
// "in", "%", "pt", "twip", "", "?" (librevenge RVNGUnit). Child vectors are
// arrays of property objects.

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

#include <libcdr/libcdr.h>
#include <librevenge-stream/librevenge-stream.h>
#include <librevenge/librevenge.h>

#define GD_BRIDGE_PROTOCOL "1"

namespace
{

void appendEscaped(std::string &out, const char *s)
{
  out += '"';
  for (const unsigned char *p = reinterpret_cast<const unsigned char *>(s); p && *p; ++p)
  {
    const unsigned char c = *p;
    switch (c)
    {
    case '"': out += "\\\""; break;
    case '\\': out += "\\\\"; break;
    case '\n': out += "\\n"; break;
    case '\r': out += "\\r"; break;
    case '\t': out += "\\t"; break;
    default:
      if (c < 0x20)
      {
        char buf[8];
        std::snprintf(buf, sizeof(buf), "\\u%04x", c);
        out += buf;
      }
      else
        out += static_cast<char>(c);
    }
  }
  out += '"';
}

const char *unitName(librevenge::RVNGUnit u)
{
  switch (u)
  {
  case librevenge::RVNG_INCH: return "in";
  case librevenge::RVNG_PERCENT: return "%";
  case librevenge::RVNG_POINT: return "pt";
  case librevenge::RVNG_TWIP: return "twip";
  case librevenge::RVNG_GENERIC: return "";
  default: return "?";
  }
}

void appendPropList(std::string &out, const librevenge::RVNGPropertyList &list);

void appendVector(std::string &out, const librevenge::RVNGPropertyListVector &vec)
{
  out += '[';
  for (unsigned long i = 0; i < vec.count(); ++i)
  {
    if (i)
      out += ',';
    appendPropList(out, vec[i]);
  }
  out += ']';
}

void appendPropList(std::string &out, const librevenge::RVNGPropertyList &list)
{
  out += '{';
  bool first = true;
  librevenge::RVNGPropertyList::Iter it(list);
  for (it.rewind(); it.next();)
  {
    if (!first)
      out += ',';
    first = false;
    appendEscaped(out, it.key());
    out += ':';
    if (it.child())
    {
      appendVector(out, *it.child());
      continue;
    }
    const librevenge::RVNGProperty *prop = it();
    if (!prop)
    {
      out += "null";
      continue;
    }
    out += '[';
    appendEscaped(out, prop->getStr().cstr());
    out += ',';
    const librevenge::RVNGUnit unit = prop->getUnit();
    appendEscaped(out, unitName(unit));
    out += ',';
    char num[64];
    std::snprintf(num, sizeof(num), "%.17g", unit == librevenge::RVNG_UNIT_ERROR ? 0.0 : prop->getDouble());
    out += num;
    out += ']';
  }
  out += '}';
}

class JsonLinesPainter : public librevenge::RVNGDrawingInterface
{
public:
  explicit JsonLinesPainter(std::FILE *out) : m_out(out) {}

  void emit(const char *op, const librevenge::RVNGPropertyList *props = nullptr)
  {
    std::string line = "{\"op\":";
    appendEscaped(line, op);
    if (props)
    {
      line += ",\"p\":";
      appendPropList(line, *props);
    }
    line += "}\n";
    std::fwrite(line.data(), 1, line.size(), m_out);
  }

#define GD_PROPS(name) void name(const librevenge::RVNGPropertyList &p) override { emit(#name, &p); }
#define GD_NOARG(name) void name() override { emit(#name); }
  GD_PROPS(startDocument)
  GD_NOARG(endDocument)
  GD_PROPS(setDocumentMetaData)
  GD_PROPS(defineEmbeddedFont)
  GD_PROPS(startPage)
  GD_NOARG(endPage)
  GD_PROPS(startMasterPage)
  GD_NOARG(endMasterPage)
  GD_PROPS(setStyle)
  GD_PROPS(startLayer)
  GD_NOARG(endLayer)
  GD_PROPS(startEmbeddedGraphics)
  GD_NOARG(endEmbeddedGraphics)
  GD_PROPS(openGroup)
  GD_NOARG(closeGroup)
  GD_PROPS(drawRectangle)
  GD_PROPS(drawEllipse)
  GD_PROPS(drawPolygon)
  GD_PROPS(drawPolyline)
  GD_PROPS(drawPath)
  GD_PROPS(drawGraphicObject)
  GD_PROPS(drawConnector)
  GD_PROPS(startTextObject)
  GD_NOARG(endTextObject)
  GD_PROPS(startTableObject)
  GD_PROPS(openTableRow)
  GD_NOARG(closeTableRow)
  GD_PROPS(openTableCell)
  GD_NOARG(closeTableCell)
  GD_PROPS(insertCoveredTableCell)
  GD_NOARG(endTableObject)
  GD_NOARG(insertTab)
  GD_NOARG(insertSpace)
  GD_NOARG(insertLineBreak)
  GD_PROPS(insertField)
  GD_PROPS(openOrderedListLevel)
  GD_PROPS(openUnorderedListLevel)
  GD_NOARG(closeOrderedListLevel)
  GD_NOARG(closeUnorderedListLevel)
  GD_PROPS(openListElement)
  GD_NOARG(closeListElement)
  GD_PROPS(defineParagraphStyle)
  GD_PROPS(openParagraph)
  GD_NOARG(closeParagraph)
  GD_PROPS(defineCharacterStyle)
  GD_PROPS(openSpan)
  GD_NOARG(closeSpan)
  GD_PROPS(openLink)
  GD_NOARG(closeLink)
#undef GD_PROPS
#undef GD_NOARG

  void insertText(const librevenge::RVNGString &text) override
  {
    std::string line = "{\"op\":\"insertText\",\"text\":";
    appendEscaped(line, text.cstr());
    line += "}\n";
    std::fwrite(line.data(), 1, line.size(), m_out);
  }

private:
  std::FILE *m_out;
};

bool readAll(std::FILE *f, std::vector<unsigned char> &data)
{
  unsigned char buf[65536];
  size_t n;
  while ((n = std::fread(buf, 1, sizeof(buf), f)) > 0)
    data.insert(data.end(), buf, buf + n);
  return !std::ferror(f);
}

} // anonymous namespace

int main(int argc, char **argv)
{
#ifdef _WIN32
  _setmode(_fileno(stdout), _O_BINARY);
  _setmode(_fileno(stdin), _O_BINARY);
#endif
  if (argc != 2)
  {
    std::fprintf(stderr, "usage: gd-cdr-bridge <file|->\n");
    return 1;
  }
  if (std::strcmp(argv[1], "--version") == 0)
  {
    std::printf("{\"op\":\"bridge\",\"protocol\":\"" GD_BRIDGE_PROTOCOL "\"}\n");
    return 0;
  }

  std::vector<unsigned char> data;
  if (std::strcmp(argv[1], "-") == 0)
  {
    if (!readAll(stdin, data))
      return 1;
  }
  else
  {
    std::FILE *f = std::fopen(argv[1], "rb");
    if (!f)
    {
      std::fprintf(stderr, "cannot open %s\n", argv[1]);
      return 1;
    }
    const bool ok = readAll(f, data);
    std::fclose(f);
    if (!ok)
      return 1;
  }
  if (data.empty())
    return 2;

  librevenge::RVNGStringStream input(data.data(), static_cast<unsigned>(data.size()));
  JsonLinesPainter painter(stdout);

  const char *format = nullptr;
  try
  {
    if (libcdr::CDRDocument::isSupported(&input))
      format = "cdr";
    else
    {
      input.seek(0, librevenge::RVNG_SEEK_SET);
      if (libcdr::CMXDocument::isSupported(&input))
        format = "cmx";
    }
  }
  catch (...)
  {
    format = nullptr;
  }
  if (!format)
    return 2;

  std::printf("{\"op\":\"bridge\",\"protocol\":\"" GD_BRIDGE_PROTOCOL "\",\"format\":\"%s\"}\n", format);
  input.seek(0, librevenge::RVNG_SEEK_SET);
  bool ok = false;
  try
  {
    ok = std::strcmp(format, "cdr") == 0 ? libcdr::CDRDocument::parse(&input, &painter)
                                         : libcdr::CMXDocument::parse(&input, &painter);
  }
  catch (...)
  {
    ok = false;
  }
  std::fflush(stdout);
  return ok ? 0 : 3;
}
