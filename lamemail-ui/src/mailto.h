#pragma once

#include <QObject>
#include <QString>
#include <QUrl>
#include <QtQml/qqmlregistration.h>

class Mailto : public QObject {
    Q_OBJECT
    QML_ELEMENT

public:
    explicit Mailto(QObject* parent = nullptr);
    ~Mailto() override;

    Q_INVOKABLE void open(const QUrl& url);

signals:
    void requested(const QString& address);
};
