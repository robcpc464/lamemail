#include "text.h"

#include <QDir>
#include <QHash>
#include <QLocale>
#include <QRegularExpression>
#include <QStringList>
#include <QUrl>
#include <algorithm>

namespace {

constexpr qsizetype longLinkChars = 60;
constexpr qsizetype shownLinkChars = 50;
constexpr char16_t ellipsis = u'\u2026';

QString clock(const QDateTime& when) { return when.toString(QStringLiteral("HH:mm")); }

bool endsInPunctuation(const QString& url)
{
    const QChar last = url.back();
    if (last == u')')
        return url.count(u'(') < url.count(u')');
    return QStringLiteral(".,;:!?'*]}~").contains(last);
}

QString withoutTrailingPunctuation(QString url)
{
    while (!url.isEmpty() && endsInPunctuation(url))
        url.chop(1);
    return url;
}

QUrl target(const QString& address)
{
    const bool bare = address.startsWith(QLatin1String("www."), Qt::CaseInsensitive);
    const QUrl url(bare ? QStringLiteral("https://") + address : address);
    return url.isValid() && !url.host().isEmpty() ? url : QUrl();
}

qsizetype hostEnd(const QString& address)
{
    static const QRegularExpression upToHost(QStringLiteral(R"(^(?:\w+://)?[^/?#]*)"));
    return upToHost.match(address).capturedEnd();
}

QString shortLink(const QString& address)
{
    if (address.size() < longLinkChars)
        return address;
    qsizetype kept = std::max(hostEnd(address), shownLinkChars);
    if (address.size() - kept < 2)
        return address;
    if (address.at(kept - 1).isHighSurrogate())
        --kept;
    return address.left(kept) + QChar(ellipsis);
}

QString appendLinked(LinkedText& linked, const QString& text)
{
    static const QRegularExpression link(
        QStringLiteral(
            R"((?<![\w.%+-])([\w.%+-]+@(?:[\w-]+\.)+[^\W\d_]{2,})|\b(?:https?://|www\.)[^\s<>"]+)"),
        QRegularExpression::CaseInsensitiveOption | QRegularExpression::UseUnicodePropertiesOption);
    QString shownText;
    qsizetype done = 0;
    for (auto matches = link.globalMatch(text); matches.hasNext();) {
        const auto match = matches.next();
        const QString mail = match.captured(1);
        const QString found = mail.isEmpty() ? withoutTrailingPunctuation(match.captured()) : mail;
        const QUrl url = mail.isEmpty() ? target(found) : QUrl(QStringLiteral("mailto:") + mail);
        if (url.isEmpty())
            continue;
        const QString address = url.toString(QUrl::FullyEncoded);
        const QString shown = mail.isEmpty() ? shortLink(found) : found;
        if (shown != found)
            linked.shortened.append(address);
        const QString before = text.mid(done, match.capturedStart() - done);
        linked.html += before.toHtmlEscaped();
        linked.html += QStringLiteral("<a href=\"%1\">%2</a>")
                           .arg(address.toHtmlEscaped(), shown.toHtmlEscaped());
        shownText += before + shown;
        done = match.capturedStart() + found.size();
    }
    linked.html += text.mid(done).toHtmlEscaped();
    return shownText + text.mid(done);
}

bool isWide(char32_t c)
{
    return (c >= 0x1100 && c <= 0x115f) || (c >= 0x2e80 && c <= 0xa4cf)
        || (c >= 0xac00 && c <= 0xd7a3) || (c >= 0xf900 && c <= 0xfaff)
        || (c >= 0xfe30 && c <= 0xfe4f) || (c >= 0xff00 && c <= 0xff60)
        || (c >= 0xffe0 && c <= 0xffe6) || (c >= 0x1f300 && c <= 0x1faff)
        || (c >= 0x20000 && c <= 0x3fffd);
}

qsizetype columns(const QString& text)
{
    qsizetype width = 0;
    for (const char32_t c : text.toUcs4()) {
        if (!QChar::isMark(c))
            width += isWide(c) ? 2 : 1;
    }
    return width;
}

bool isRow(const QString& line)
{
    return line.size() > 1 && line.startsWith(u'|') && line.endsWith(u'|');
}

bool isRule(const QString& line)
{
    static const QRegularExpression rule(QStringLiteral(R"(^\|(?:-+\|)+$)"));
    return rule.match(line).hasMatch();
}

qsizetype tableEnd(const QStringList& lines, qsizetype at)
{
    if (at + 1 >= lines.size() || !isRow(lines.at(at)) || !isRule(lines.at(at + 1)))
        return at;
    qsizetype end = at + 2;
    while (end < lines.size() && isRow(lines.at(end)))
        ++end;
    return end;
}

QStringList cells(const QString& row)
{
    QStringList cells;
    QString cell;
    for (qsizetype at = 1; at < row.size() - 1; ++at) {
        if (row.at(at) == u'|' && row.at(at - 1) != u'\\') {
            cells.append(cell.trimmed());
            cell.clear();
        } else {
            cell += row.at(at);
        }
    }
    cells.append(cell.trimmed());
    return cells;
}

struct Cell {
    QString html;
    qsizetype width;
};

void appendTable(LinkedText& linked, const QStringList& lines)
{
    QList<QList<Cell>> rows;
    QList<qsizetype> widths;
    for (qsizetype at = 0; at < lines.size(); ++at) {
        QList<Cell> row;
        if (at != 1) {
            for (const QString& text : cells(lines.at(at))) {
                LinkedText cell;
                const qsizetype width = columns(appendLinked(cell, text));
                linked.shortened += cell.shortened;
                if (row.size() == widths.size())
                    widths.append(0);
                widths[row.size()] = std::max(widths.at(row.size()), width);
                row.append({ cell.html, width });
            }
        }
        rows.append(row);
    }
    for (qsizetype at = 0; at < rows.size(); ++at) {
        if (at > 0)
            linked.html += u'\n';
        linked.html += u'|';
        if (at == 1) {
            for (const qsizetype width : widths)
                linked.html += QString(width + 2, u'-') + u'|';
            continue;
        }
        const QList<Cell>& row = rows.at(at);
        for (qsizetype column = 0; column < row.size(); ++column) {
            const Cell& cell = row.at(column);
            linked.html += u' ' + cell.html + QString(widths.at(column) - cell.width, u' ')
                + QStringLiteral(" |");
        }
    }
}

}

QString person(const Addr& addr)
{
    const QString name = addr.name.trimmed();
    const QString address = addr.address.trimmed();
    if (!name.isEmpty() && !address.isEmpty())
        return QStringLiteral("%1 <%2>").arg(name, address);
    return name.isEmpty() ? address : name;
}

QString displayName(const Addr& addr)
{
    const QString name = addr.name.trimmed();
    return name.isEmpty() ? addr.address.trimmed() : name;
}

QString personMarkup(const Addr& addr)
{
    const QString name = addr.name.trimmed();
    const QString address = addr.address.trimmed();
    if (name.isEmpty() || address.isEmpty())
        return QStringLiteral("<b>%1</b>").arg((name.isEmpty() ? address : name).toHtmlEscaped());
    return QStringLiteral("<b>%1</b> <small><font color=\"{dim}\">%2</font></small>")
        .arg(name.toHtmlEscaped(), address.toHtmlEscaped());
}

QString people(const QList<Addr>& list, QString (*format)(const Addr&))
{
    QStringList names;
    for (const Addr& addr : list)
        names.append(format(addr));
    return names.join(QStringLiteral(", "));
}

std::optional<QDateTime> parseTime(const QString& received)
{
    const QDateTime when = QDateTime::fromString(received, Qt::ISODate);
    if (!when.isValid())
        return std::nullopt;
    return when.toLocalTime();
}

QString shortTime(const QDateTime& when, const QDateTime& now, Lang lang)
{
    const Texts& t = texts(lang);
    const qint64 secs = when.secsTo(now);
    if (secs >= 0 && secs < 60)
        return QString::fromUtf8(t.now);
    if (secs >= 60 && secs < 3600)
        return QString::fromUtf8(t.minutesAgo)
            .replace(QLatin1String("{}"), QString::number(secs / 60));
    const QDate day = when.date();
    const qint64 days = day.daysTo(now.date());
    if (days <= 0)
        return clock(when);
    if (days == 1)
        return QString::fromUtf8(t.yesterday);
    if (days < 7)
        return QString::fromUtf8(t.weekdays[day.dayOfWeek() - 1]);
    const QString month = QString::fromUtf8(t.months[day.month() - 1]);
    if (day.year() == now.date().year())
        return lang == Lang::En ? QStringLiteral("%1 %2").arg(month).arg(day.day())
                                : QStringLiteral("%1. %2").arg(day.day()).arg(month);
    return dateText(day, t.dateLengths[0].format, lang);
}

QString detailTime(const QDateTime& when, const QDateTime& now, Lang lang)
{
    if (when.date().daysTo(now.date()) <= 0)
        return clock(when);
    return shortTime(when, now, lang) + QStringLiteral(" · ") + clock(when);
}

QString dateText(const QDate& date, const char* format, Lang lang)
{
    return QLocale(lang == Lang::De ? QLocale::German : QLocale::English)
        .toString(date, QString::fromUtf8(format));
}

int dayIndex(const std::optional<QDateTime>& when, const QDate& today)
{
    return int(std::clamp<qint64>(when ? when->date().daysTo(today) : 2, 0, 2));
}

QString fileSize(qint64 bytes)
{
    constexpr qint64 kb = 1024;
    constexpr qint64 mb = kb * kb;
    if (bytes < kb)
        return QStringLiteral("%1 B").arg(bytes);
    if (bytes < 10 * mb)
        return QStringLiteral("%1 KB").arg((bytes + kb - 1) / kb);
    return QStringLiteral("%1 MB").arg((bytes + mb - 1) / mb);
}

QString tildePath(const QString& path)
{
    const QString home = QDir::homePath();
    if (path == home)
        return QStringLiteral("~");
    if (path.startsWith(home + QLatin1Char('/')))
        return QLatin1Char('~') + path.mid(home.size());
    return path;
}

LinkedText linkedText(const QString& text)
{
    LinkedText linked { QStringLiteral("<span style=\"white-space: pre-wrap\">"), { } };
    const QStringList lines = text.split(u'\n');
    for (qsizetype at = 0; at < lines.size();) {
        if (at > 0)
            linked.html += u'\n';
        const qsizetype end = tableEnd(lines, at);
        if (end > at) {
            appendTable(linked, lines.mid(at, end - at));
            at = end;
        } else {
            appendLinked(linked, lines.at(at++));
        }
    }
    linked.html += QStringLiteral("</span>");
    return linked;
}

namespace {

QString filled(const char* text, const QStringList& values)
{
    QString result = QString::fromUtf8(text);
    for (const QString& value : values) {
        const qsizetype at = result.indexOf(QLatin1String("{}"));
        if (at >= 0)
            result.replace(at, 2, value);
    }
    return result;
}

QString usedLine(const UsedMail& mail, const QDateTime& now, Lang lang)
{
    const auto when = parseTime(mail.date);
    const QString subject = mail.subject.isEmpty() ? QStringLiteral("—") : mail.subject;
    return subject + QStringLiteral("  ·  ") + (when ? shortTime(*when, now, lang) : mail.date);
}

QStringList contextLines(const std::optional<QList<ContextEntry>>& context, const Texts& t)
{
    if (!context)
        return { QString::fromUtf8(t.aiReportNoFolderLine) };
    if (context->isEmpty())
        return { QString::fromUtf8(t.aiReportEmptyFolder) };
    const QHash<QString, const char*> skipped {
        { QStringLiteral("not_text"), t.aiContextNotText },
        { QStringLiteral("empty"), t.aiContextEmptyFile },
        { QStringLiteral("limit"), t.aiContextLimit },
        { QStringLiteral("folder"), t.aiContextFolder },
    };
    QStringList lines;
    for (const ContextEntry& entry : *context) {
        if (entry.state == QLatin1String("cut"))
            lines.append(entry.name + QStringLiteral(" (") + QString::fromUtf8(t.aiContextCut)
                + QLatin1Char(')'));
        else if (const char* reason = skipped.value(entry.state))
            lines.append(entry.name + QStringLiteral(" (")
                + filled(t.aiReportSkipped, { QString::fromUtf8(reason) }) + QLatin1Char(')'));
        else
            lines.append(entry.name);
    }
    return lines;
}

QVariantMap section(const QString& title, const QStringList& lines)
{
    return { { QStringLiteral("title"), title }, { QStringLiteral("lines"), lines } };
}

}

QVariantMap aiReport(const Suggestion& suggestion, const QDateTime& now, Lang lang)
{
    const Texts& t = texts(lang);
    const auto lines = [&](const QList<UsedMail>& mails) {
        QStringList lines;
        for (const UsedMail& mail : mails)
            lines.append(usedLine(mail, now, lang));
        return lines;
    };
    QStringList summary;
    if (const auto& context = suggestion.context) {
        const auto used = std::count_if(
            context->begin(), context->end(), [](const ContextEntry& entry) {
                return entry.state == QLatin1String("read") || entry.state == QLatin1String("cut");
            });
        summary.append(
            filled(t.aiReportFiles, { QString::number(used), QString::number(context->size()) }));
    } else {
        summary.append(QString::fromUtf8(t.aiReportNoFolder));
    }
    QVariantList sections { section(
        QString::fromUtf8(t.aiReportFolderTitle), contextLines(suggestion.context, t)) };
    if (!suggestion.searches.isEmpty()) {
        summary.append(filled(t.aiReportHits, { QString::number(suggestion.found.size()) }));
        const QStringList found = lines(suggestion.found);
        sections.append(section(
            filled(t.aiReportSearchTitle, { suggestion.searches.join(QStringLiteral(", ")) }),
            found.isEmpty() ? QStringList { QString::fromUtf8(t.aiReportNoHits) } : found));
    }
    if (!suggestion.earlier.isEmpty()) {
        summary.append(filled(t.aiReportEarlier, { QString::number(suggestion.earlier.size()) }));
        sections.append(
            section(QString::fromUtf8(t.aiReportEarlierTitle), lines(suggestion.earlier)));
    }
    return { { QStringLiteral("summary"), summary.join(QStringLiteral("  ·  ")) },
        { QStringLiteral("sections"), sections } };
}
