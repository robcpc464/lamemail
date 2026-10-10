#pragma once

#include <QDBusMessage>
#include <QHash>
#include <QObject>
#include <QString>
#include <QUrl>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

QString portalRequestPath(const QString& sender, const QString& token);

class Opener : public QObject {
    Q_OBJECT
    QML_ELEMENT

public:
    using QObject::QObject;

    Q_INVOKABLE void open(const QUrl& url);

signals:
    void failed(const QString& message);

private slots:
    void responded(uint code, const QVariantMap& results, const QDBusMessage& message);

private:
    bool openWithPortal(const QUrl& url);
    QString forget(const QString& path);

    QHash<QString, QString> m_names;
    int m_requests = 0;
};
