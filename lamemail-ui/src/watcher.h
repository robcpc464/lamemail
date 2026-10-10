#pragma once

#include <QHash>
#include <QObject>
#include <QSocketNotifier>
#include <QString>
#include <QTimer>
#include <QtQml/qqmlregistration.h>

bool isInside(const QString& path, const QString& dir);

class Watcher : public QObject {
    Q_OBJECT
    QML_ELEMENT

public:
    explicit Watcher(QObject* parent = nullptr);
    ~Watcher() override;

    Q_INVOKABLE void start();

signals:
    void changed();
    void themeChanged();

private:
    void addTree(const QString& dir);
    void read();

    int m_fd = -1;
    QSocketNotifier* m_notifier = nullptr;
    QHash<int, QString> m_dirs;
    QString m_mail;
    QString m_theme;
    QTimer m_quiet;
    bool m_mailHit = false;
    bool m_themeHit = false;
};
