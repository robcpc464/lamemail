#pragma once

#include <QObject>
#include <QString>
#include <QUrl>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

class Mailto : public QObject {
    Q_OBJECT
    QML_ELEMENT

public:
    explicit Mailto(QObject* parent = nullptr);
    ~Mailto() override;

    static QString link(const QString& argument);
    Q_INVOKABLE QVariantMap parse(const QUrl& url) const;
    Q_INVOKABLE void open(const QUrl& url);

signals:
    void requested(const QVariantMap& mail);
};
