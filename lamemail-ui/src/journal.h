#pragma once

#include "lamemail.h"

#include <QByteArray>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QtQml/qqmlregistration.h>

class Journal : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(int count READ count NOTIFY changed)
    Q_PROPERTY(int sending READ sending NOTIFY changed)
    Q_PROPERTY(QStringList leaving READ leaving NOTIFY changed)
    Q_PROPERTY(QStringList answering READ answering NOTIFY changed)
    Q_PROPERTY(bool pushing READ pushing NOTIFY pushingChanged)
    Q_PROPERTY(QString failure READ failure NOTIFY failureChanged)

public:
    explicit Journal(QObject* parent = nullptr);

    int count() const { return m_count; }
    int sending() const { return m_sending; }
    QStringList leaving() const { return m_leaving; }
    QStringList answering() const { return m_answering; }
    bool pushing() const { return m_pushing; }
    QString failure() const { return m_failure; }

    Q_INVOKABLE void reload();
    Q_INVOKABLE void push();

signals:
    void changed();
    void pushingChanged();
    void failureChanged();
    void dropped(const QString& reason, const QString& messageId);
    void sent(const QString& messageId, const QString& message, bool complete);

private:
    void report(const lamemail::Outcome& outcome);
    void setPushing(bool pushing);
    void setFailure(const QString& failure);

    QTimer m_soon;
    QByteArray m_entries;
    QString m_failure;
    QStringList m_leaving;
    QStringList m_answering;
    int m_count = 0;
    int m_sending = 0;
    bool m_loading = false;
    bool m_again = false;
    bool m_pushing = false;
    bool m_pushAgain = false;
};
