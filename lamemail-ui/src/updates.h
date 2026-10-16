#pragma once

#include "entry.h"

#include <QObject>
#include <QString>
#include <QTimer>
#include <QtQml/qqmlregistration.h>
#include <optional>

class Updates : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool newer READ newer NOTIFY changed)
    Q_PROPERTY(QString latest READ latest NOTIFY changed)
    Q_PROPERTY(QString url READ url NOTIFY changed)

public:
    explicit Updates(QObject* parent = nullptr);

    bool newer() const { return m_release && m_release->newer; }
    QString latest() const { return m_release ? m_release->latest : QString(); }
    QString url() const { return m_release ? m_release->url : QString(); }

    Q_INVOKABLE void check();

signals:
    void changed();

private:
    bool due() const;

    std::optional<Release> m_release;
    QTimer m_timer;
    bool m_checking = false;
};
