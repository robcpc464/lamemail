#include "mailto.h"

#include <QDesktopServices>
#include <QUrlQuery>
#include <QVariantList>

namespace {

void take(QVariantList& people, const QString& list, const QString& kind)
{
    for (const QString& entry : list.split(u',', Qt::SkipEmptyParts)) {
        const QString text = entry.trimmed();
        const qsizetype open = text.lastIndexOf(u'<');
        const bool named = open >= 0 && text.endsWith(u'>');
        const QString address = named ? text.mid(open + 1).chopped(1).trimmed() : text;
        if (address.isEmpty())
            continue;
        people.append(QVariantMap {
            { QStringLiteral("name"), named ? text.left(open).remove(u'"').trimmed() : QString() },
            { QStringLiteral("address"), address },
            { QStringLiteral("kind"), kind },
        });
    }
}

}

Mailto::Mailto(QObject* parent)
    : QObject(parent)
{
    QDesktopServices::setUrlHandler(QStringLiteral("mailto"), this, "open");
}

Mailto::~Mailto() { QDesktopServices::unsetUrlHandler(QStringLiteral("mailto")); }

QString Mailto::link(const QString& argument)
{
    const QUrl url(argument.trimmed());
    return url.scheme() == QLatin1String("mailto") ? QString::fromLatin1(url.toEncoded())
                                                   : QString();
}

QVariantMap Mailto::parse(const QUrl& url) const
{
    QVariantList people;
    QString subject;
    QString body;
    take(people, url.path(QUrl::FullyDecoded), QStringLiteral("to"));
    for (const auto& [key, value] : QUrlQuery(url).queryItems(QUrl::FullyDecoded)) {
        const QString name = key.toLower();
        if (name == QLatin1String("to") || name == QLatin1String("cc")
            || name == QLatin1String("bcc"))
            take(people, value, name);
        else if (name == QLatin1String("subject"))
            subject = value;
        else if (name == QLatin1String("body"))
            body = QString(value).replace(QLatin1String("\r\n"), QLatin1String("\n"));
    }
    return {
        { QStringLiteral("people"), people },
        { QStringLiteral("subject"), subject },
        { QStringLiteral("body"), body },
    };
}

void Mailto::open(const QUrl& url) { emit requested(parse(url)); }
