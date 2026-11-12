#pragma once

#include <QObject>
#include <QString>
#include <QtQml/qqmlregistration.h>

class Checker : public QObject {
    Q_OBJECT
    QML_ELEMENT

public:
    using QObject::QObject;

    Q_INVOKABLE void check(const QString& path);

signals:
    void checked(const QString& path, bool suspicious, const QString& kind, const QString& reason);
    void dropped(const QString& path);

private:
    void run(const QString& path);

    QString m_running;
    QString m_waiting;
};
