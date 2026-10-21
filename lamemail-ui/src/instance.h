#pragma once

#include <QByteArray>
#include <QLocalServer>
#include <QObject>
#include <QString>

class Instance : public QObject {
    Q_OBJECT

public:
    explicit Instance(const QString& dir, QObject* parent = nullptr);

    bool handOver(const QByteArray& token, const QString& path);
    bool listen();

signals:
    void activated(const QByteArray& token, const QString& path);

private:
    QString m_name;
    QLocalServer m_server;
};
