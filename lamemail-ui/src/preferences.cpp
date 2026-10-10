#include "preferences.h"

#include "text.h"

#include <QDir>
#include <QSettings>
#include <QStandardPaths>

namespace {

const QString wheelKey = QStringLiteral("preferences/wheelPixels");
const QString trayKey = QStringLiteral("preferences/closeToTray");
const QString flaggedFirstKey = QStringLiteral("preferences/flaggedFirst");
const QString feedKindsKey = QStringLiteral("preferences/feedKinds");
const QString shadowsKey = QStringLiteral("preferences/shadows");
const QString notificationsKey = QStringLiteral("preferences/notifications");
const QString themeKey = QStringLiteral("preferences/theme");
const QString downloadsKey = QStringLiteral("preferences/downloadFolder");
constexpr int defaultWheelPixels = 120;
const QStringList defaultFeedKinds { QStringLiteral("inbox"), QStringLiteral("sent"),
    QStringLiteral("drafts") };
bool effectsModule = false;

}

int Preferences::wheelPixels() const
{
    return QSettings().value(wheelKey, defaultWheelPixels).toInt();
}

void Preferences::setWheelPixels(int pixels)
{
    if (pixels == wheelPixels())
        return;
    QSettings().setValue(wheelKey, pixels);
    emit wheelPixelsChanged();
}

bool Preferences::closeToTray() const { return QSettings().value(trayKey, false).toBool(); }

void Preferences::setCloseToTray(bool on)
{
    if (on == closeToTray())
        return;
    QSettings().setValue(trayKey, on);
    emit closeToTrayChanged();
}

bool Preferences::flaggedFirst() const { return QSettings().value(flaggedFirstKey, true).toBool(); }

void Preferences::setFlaggedFirst(bool on)
{
    if (on == flaggedFirst())
        return;
    QSettings().setValue(flaggedFirstKey, on);
    emit flaggedFirstChanged();
}

QStringList Preferences::feedKinds() const
{
    const QStringList kinds = QSettings().value(feedKindsKey).toStringList();
    return kinds.isEmpty() ? defaultFeedKinds : kinds;
}

void Preferences::setFeedKinds(const QStringList& kinds)
{
    if (kinds.isEmpty() || kinds == feedKinds())
        return;
    QSettings().setValue(feedKindsKey, kinds);
    emit feedKindsChanged();
}

bool Preferences::shadows() const { return QSettings().value(shadowsKey, true).toBool(); }

void Preferences::setShadows(bool on)
{
    if (on == shadows())
        return;
    QSettings().setValue(shadowsKey, on);
    emit shadowsChanged();
}

bool Preferences::shadowsAvailable() { return effectsModule; }

void Preferences::setShadowsAvailable(bool available) { effectsModule = available; }

bool Preferences::notifications() const
{
    return QSettings().value(notificationsKey, true).toBool();
}

void Preferences::setNotifications(bool on)
{
    if (on == notifications())
        return;
    QSettings().setValue(notificationsKey, on);
    emit notificationsChanged();
}

QString Preferences::theme()
{
    return QSettings().value(themeKey, QStringLiteral("system")).toString();
}

void Preferences::setTheme(const QString& name)
{
    if (name == theme())
        return;
    QSettings().setValue(themeKey, name);
    emit themeChanged();
}

QString Preferences::downloadFolder() { return QSettings().value(downloadsKey).toString(); }

void Preferences::setDownloadFolder(const QString& folder)
{
    const QString chosen = folder.trimmed();
    if (chosen == downloadFolder())
        return;
    QSettings().setValue(downloadsKey, chosen);
    emit downloadFolderChanged();
}

QString Preferences::defaultDownloadFolder() { return tildePath(downloadsPath({ })); }

QString Preferences::downloads() { return downloadsPath(downloadFolder()); }

QString downloadsPath(const QString& chosen)
{
    QString path = chosen.trimmed();
    if (path.isEmpty())
        return QStandardPaths::writableLocation(QStandardPaths::DownloadLocation);
    if (path == QLatin1Char('~') || path.startsWith(QLatin1String("~/")))
        path.replace(0, 1, QDir::homePath());
    return QDir::cleanPath(QDir::home().absoluteFilePath(path));
}
