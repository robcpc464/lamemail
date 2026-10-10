#pragma once

#include "lamemail.h"

#include <QObject>
#include <QString>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>
#include <functional>

class Sender : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(int noting READ noting NOTIFY notingChanged)

public:
    using QObject::QObject;

    int noting() const { return m_noting; }
    Q_INVOKABLE void send(const QVariantMap& job);
    Q_INVOKABLE void save(const QVariantMap& job);
    Q_INVOKABLE void discard(const QVariantMap& job);
    Q_INVOKABLE void deleteForGood(const QString& path);

signals:
    void notingChanged();
    void finished(const QVariantMap& job, const QString& messageId, const QString& error);
    void saved(const QVariantMap& job, const QString& messageId, const QString& error);
    void discarded(const QVariantMap& job, const QString& error);
    void deletedForGood(const QString& path, const QString& error);

private:
    void note(const QString& verb, const QVariantMap& job,
        void (Sender::*done)(const QVariantMap&, const QString&, const QString&));
    void run(const QStringList& command, const QByteArray& input,
        const std::function<void(const lamemail::Outcome&)>& done);

    int m_noting = 0;
};
