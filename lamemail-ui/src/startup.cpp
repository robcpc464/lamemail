#include "startup.h"

#include "preferences.h"
#include "tray.h"

#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlComponent>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QUrl>

namespace lamemail {

void identify()
{
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    QGuiApplication::setOrganizationName(QStringLiteral("lamemail"));
    QGuiApplication::setApplicationName(QStringLiteral("lamemail-ui"));
    QGuiApplication::setDesktopFileName(QStringLiteral("lamemail-ui"));
    QGuiApplication::setApplicationVersion(QStringLiteral("0.7.0"));
}

QCommandLineOption dirOption()
{
    return { QStringLiteral("dir"),
        QStringLiteral("Sync folder (default: $LAMEMAIL_DIR or ~/.lamemail)"),
        QStringLiteral("DIR") };
}

QQuickWindow* open(QQmlApplicationEngine& engine, bool hidden)
{
    QQmlComponent effects(&engine);
    effects.setData("import QtQuick\nimport QtQuick.Effects\nMultiEffect {}\n",
        QUrl(QStringLiteral("effects.qml")));
    Preferences::setShadowsAvailable(effects.isReady());
    engine.addImageProvider(QStringLiteral("tray"), new TrayImages);
    engine.setInitialProperties({ { QStringLiteral("startHidden"), hidden } });
    engine.load(QUrl(QStringLiteral("qrc:/qml/Main.qml")));
    const QList<QObject*> roots = engine.rootObjects();
    return roots.isEmpty() ? nullptr : qobject_cast<QQuickWindow*>(roots.constFirst());
}

}
