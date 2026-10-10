#include "mailto.h"

#include <QDesktopServices>

Mailto::Mailto(QObject* parent)
    : QObject(parent)
{
    QDesktopServices::setUrlHandler(QStringLiteral("mailto"), this, "open");
}

Mailto::~Mailto() { QDesktopServices::unsetUrlHandler(QStringLiteral("mailto")); }

void Mailto::open(const QUrl& url)
{
    const QString address = url.path(QUrl::FullyDecoded).trimmed();
    if (!address.isEmpty())
        emit requested(address);
}
