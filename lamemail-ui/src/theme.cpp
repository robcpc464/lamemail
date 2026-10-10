#include "theme.h"
#include "preferences.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QProcess>

namespace {

constexpr int defaultFontSize = 12;

Toml readToml(const QString& name)
{
    const QString current = omarchyCurrent();
    if (current.isEmpty())
        return { };
    QFile file(current + QStringLiteral("/theme/") + name);
    if (!file.open(QIODevice::ReadOnly))
        return { };
    return parseToml(QString::fromUtf8(file.readAll()));
}

QString monospaceFamily()
{
    QProcess fcMatch;
    fcMatch.start(QStringLiteral("fc-match"),
        { QStringLiteral("-f"), QStringLiteral("%{family[0]}"), QStringLiteral("monospace") });
    if (fcMatch.waitForFinished(2000) && fcMatch.exitStatus() == QProcess::NormalExit
        && fcMatch.exitCode() == 0) {
        const QString family = QString::fromUtf8(fcMatch.readAllStandardOutput()).trimmed();
        if (!family.isEmpty())
            return family;
    }
    return QStringLiteral("monospace");
}

}

Toml parseToml(const QString& text)
{
    Toml table;
    QString section;
    for (const QString& raw : text.split(QLatin1Char('\n'))) {
        const QString line = raw.trimmed();
        if (line.isEmpty() || line.startsWith(QLatin1Char('#')))
            continue;
        if (line.startsWith(QLatin1Char('['))) {
            section = line;
            section.remove(QLatin1Char('[')).remove(QLatin1Char(']'));
            section = section.trimmed();
            continue;
        }
        const qsizetype equals = line.indexOf(QLatin1Char('='));
        if (equals < 0)
            continue;
        const QString key = line.left(equals).trimmed();
        QString value = line.mid(equals + 1).trimmed();
        if (value.startsWith(QLatin1Char('"'))) {
            const qsizetype end = value.indexOf(QLatin1Char('"'), 1);
            value = end < 0 ? value.mid(1) : value.mid(1, end - 1);
        } else {
            const qsizetype hash = value.indexOf(QLatin1Char('#'));
            if (hash >= 0)
                value = value.left(hash).trimmed();
        }
        table.insert(section.isEmpty() ? key : section + QLatin1Char('.') + key, value);
    }
    return table;
}

std::optional<QColor> hexColor(const QString& text)
{
    const QString trimmed = text.trimmed();
    if (trimmed.size() != 7 || !trimmed.startsWith(QLatin1Char('#')))
        return std::nullopt;
    bool ok = false;
    const uint rgb = trimmed.mid(1).toUInt(&ok, 16);
    if (!ok)
        return std::nullopt;
    return QColor::fromRgb(rgb >> 16 & 0xff, rgb >> 8 & 0xff, rgb & 0xff);
}

QColor pickColor(const Toml& table, const QStringList& keys, const QString& fallback)
{
    for (const QString& key : keys) {
        if (const auto color = hexColor(table.value(key)))
            return *color;
    }
    return hexColor(fallback).value_or(QColor());
}

QString omarchyCurrent()
{
    const QString dir = QDir::homePath() + QStringLiteral("/.local/state/omarchy/current");
    return QFileInfo(dir).isDir() ? dir : QString();
}

Theme::Theme(QObject* parent)
    : QObject(parent)
    , m_palette(load())
{
}

void Theme::reload()
{
    m_palette = load();
    emit changed();
}

Theme::Palette Theme::load()
{
    const QString chosen = Preferences::theme();
    const Toml colors = chosen == QLatin1String("system")
        ? readToml(QStringLiteral("colors.toml"))
        : Toml { { QStringLiteral("mode"), chosen } };
    const Toml shell = readToml(QStringLiteral("shell.toml"));
    const bool dark = colors.value(QStringLiteral("mode")) != QLatin1String("light");
    auto pick = [&](const QStringList& keys, const char* darkFallback, const char* lightFallback) {
        return pickColor(colors, keys, QLatin1String(dark ? darkFallback : lightFallback));
    };
    bool ok = false;
    const int fontSize = shell.value(QStringLiteral("font.base-size")).toInt(&ok);
    return {
        .background = pick({ "background" }, "#1e1e2e", "#ffffff"),
        .backgroundDeep
        = pick({ "darker_background", "dark_background", "background" }, "#101019", "#e6e9ef"),
        .surface = pick({ "lighter_background", "background" }, "#313244", "#ccd0da"),
        .foreground = pick({ "foreground" }, "#cdd6f4", "#4c4f69"),
        .foregroundBright = pick({ "bright_foreground", "foreground" }, "#ffffff", "#303446"),
        .muted = pick({ "dark_foreground", "muted" }, "#6c7086", "#8c8fa1"),
        .line = pick({ "muted", "selection" }, "#585b70", "#9ca0b0"),
        .accent = pick({ "accent", "blue" }, "#89b4fa", "#1e66f5"),
        .red = pick({ "red" }, "#f38ba8", "#d20f39"),
        .yellow = pick({ "yellow" }, "#f9e2af", "#df8e1d"),
        .orange = pick({ "orange", "yellow" }, "#f6b6ab", "#fe640b"),
        .green = pick({ "green" }, "#a6e3a1", "#40a02b"),
        .cyan = pick({ "cyan" }, "#94e2d5", "#179299"),
        .blue = pick({ "blue" }, "#89b4fa", "#1e66f5"),
        .magenta = pick({ "magenta" }, "#f5c2e7", "#ea76cb"),
        .fontFamily = monospaceFamily(),
        .fontSize = ok && fontSize > 0 ? fontSize : defaultFontSize,
    };
}
