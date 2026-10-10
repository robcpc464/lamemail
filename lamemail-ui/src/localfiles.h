#pragma once

#include <QList>
#include <QObject>
#include <QUrl>
#include <QVariantList>
#include <QtQml/qqmlregistration.h>

class LocalFiles : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

public:
    using QObject::QObject;

    Q_INVOKABLE QVariantList describe(const QList<QUrl>& urls) const;
};
