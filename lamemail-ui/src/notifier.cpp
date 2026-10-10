#include "notifier.h"

#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDebug>
#include <QStringList>
#include <QVariantMap>

namespace {

constexpr qsizetype oneByOne = 2;
const QString service = QStringLiteral("org.freedesktop.Notifications");
const QString objectPath = QStringLiteral("/org/freedesktop/Notifications");
const QString defaultAction = QStringLiteral("default");

QString field(const QVariant& mail, const char* key)
{
    return mail.toMap().value(QLatin1String(key)).toString();
}

}

QList<Note> notes(const QVariantList& mails, Lang lang)
{
    QList<Note> list;
    if (mails.size() <= oneByOne) {
        for (const QVariant& mail : mails)
            list.append({ field(mail, "sender"), field(mail, "subject"), field(mail, "path") });
        return list;
    }
    QStringList senders;
    for (const QVariant& mail : mails) {
        const QString sender = field(mail, "sender");
        if (!sender.isEmpty() && !senders.contains(sender))
            senders.append(sender);
    }
    list.append({ QString::fromUtf8(texts(lang).newMails)
                      .replace(QLatin1String("{}"), QString::number(mails.size())),
        senders.join(QLatin1String(", ")), { } });
    return list;
}

Notifier::Notifier(QObject* parent)
    : QObject(parent)
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected())
        return;
    bus.connect(service, objectPath, service, QStringLiteral("ActionInvoked"), this,
        SLOT(invoked(uint, QString)));
    bus.connect(service, objectPath, service, QStringLiteral("ActivationToken"), this,
        SLOT(activating(uint, QString)));
    bus.connect(service, objectPath, service, QStringLiteral("NotificationClosed"), this,
        SLOT(closed(uint, uint)));
}

void Notifier::announce(const QVariantList& mails)
{
    if (!QDBusConnection::sessionBus().isConnected())
        return;
    for (const Note& note : notes(mails, lang()))
        show(note);
}

void Notifier::show(const Note& note)
{
    QDBusMessage call
        = QDBusMessage::createMethodCall(service, objectPath, service, QStringLiteral("Notify"));
    call << QStringLiteral("lamemail") << 0u << QStringLiteral("lamemail") << note.title
         << note.body.toHtmlEscaped()
         << QStringList { defaultAction, QString::fromUtf8(texts().open) }
         << QVariantMap { { QStringLiteral("desktop-entry"), QStringLiteral("lamemail-ui") },
                { QStringLiteral("category"), QStringLiteral("email.arrived") } }
         << -1;
    auto* watcher
        = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(call), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, watcher, path = note.path] {
        watcher->deleteLater();
        const QDBusPendingReply<uint> reply = *watcher;
        if (reply.isError())
            qWarning().noquote() << "lamemail-ui: notification:" << reply.error().message();
        else
            m_paths.insert(reply.value(), path);
    });
}

void Notifier::invoked(uint id, const QString& action)
{
    if (action == defaultAction && m_paths.contains(id))
        emit opened(m_paths.value(id));
}

void Notifier::activating(uint id, const QString& token)
{
    if (m_paths.contains(id))
        qputenv("XDG_ACTIVATION_TOKEN", token.toUtf8());
}

void Notifier::closed(uint id, uint) { m_paths.remove(id); }
