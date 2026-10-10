#include "opener.h"

#include "texts.h"

#include <QDBusConnection>
#include <QDBusPendingCallWatcher>
#include <QDBusUnixFileDescriptor>
#include <QDesktopServices>
#include <QFile>

namespace {

const QString service = QStringLiteral("org.freedesktop.portal.Desktop");
const QString objectPath = QStringLiteral("/org/freedesktop/portal/desktop");
const QString openUri = QStringLiteral("org.freedesktop.portal.OpenURI");
const QString request = QStringLiteral("org.freedesktop.portal.Request");
const QString response = QStringLiteral("Response");
const char* const respondedSlot = SLOT(responded(uint, QVariantMap, QDBusMessage));
constexpr uint portalFailed = 2;

}

QString portalRequestPath(const QString& sender, const QString& token)
{
    QString name = sender;
    name.remove(QLatin1Char(':')).replace(QLatin1Char('.'), QLatin1Char('_'));
    return objectPath + QStringLiteral("/request/") + name + QLatin1Char('/') + token;
}

void Opener::open(const QUrl& url)
{
    if (!openWithPortal(url))
        QDesktopServices::openUrl(url);
}

bool Opener::openWithPortal(const QUrl& url)
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    QFile file(url.toLocalFile());
    if (qEnvironmentVariableIsSet("QT_NO_XDG_DESKTOP_PORTAL") || !bus.isConnected()
        || !(bus.connectionCapabilities() & QDBusConnection::UnixFileDescriptorPassing)
        || !file.open(QIODevice::ReadOnly))
        return false;
    const QString token = QStringLiteral("lamemail%1").arg(++m_requests);
    const QString path = portalRequestPath(bus.baseService(), token);
    bus.connect(service, path, request, response, this, respondedSlot);
    m_names.insert(path, url.fileName());
    QDBusMessage call
        = QDBusMessage::createMethodCall(service, objectPath, openUri, QStringLiteral("OpenFile"));
    call << QString() << QVariant::fromValue(QDBusUnixFileDescriptor(file.handle()))
         << QVariantMap { { QStringLiteral("handle_token"), token } };
    auto* watcher = new QDBusPendingCallWatcher(bus.asyncCall(call), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, watcher, url, path] {
        watcher->deleteLater();
        if (!watcher->isError())
            return;
        forget(path);
        QDesktopServices::openUrl(url);
    });
    return true;
}

void Opener::responded(uint code, const QVariantMap&, const QDBusMessage& message)
{
    const QString name = forget(message.path());
    if (code == portalFailed)
        emit failed(QString::fromUtf8(texts().openFailed).replace(QLatin1String("{}"), name));
}

QString Opener::forget(const QString& path)
{
    QDBusConnection::sessionBus().disconnect(service, path, request, response, this, respondedSlot);
    return m_names.take(path);
}
