#pragma once

#include "texts.h"

#include <QHash>
#include <QList>
#include <QObject>
#include <QString>
#include <QVariantList>
#include <QtQml/qqmlregistration.h>

struct Note {
    QString title;
    QString body;
    QString path;

    bool operator==(const Note&) const = default;
};

QList<Note> notes(const QVariantList& mails, Lang lang);

class Notifier : public QObject {
    Q_OBJECT
    QML_ELEMENT

public:
    explicit Notifier(QObject* parent = nullptr);

    Q_INVOKABLE void announce(const QVariantList& mails);

signals:
    void opened(const QString& path);

private slots:
    void invoked(uint id, const QString& action);
    void activating(uint id, const QString& token);
    void closed(uint id, uint reason);

private:
    void show(const Note& note);

    QHash<uint, QString> m_paths;
};
