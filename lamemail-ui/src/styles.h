#pragma once

#include <QObject>
#include <QString>
#include <QStringList>
#include <QtQml/qqmlregistration.h>

class Styles : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QStringList icons READ icons CONSTANT)
    Q_PROPERTY(QStringList flags READ flags CONSTANT)
    Q_PROPERTY(QStringList colors READ colors CONSTANT)
    Q_PROPERTY(QStringList kinds READ kinds CONSTANT)
    Q_PROPERTY(QStringList mailKinds READ mailKinds CONSTANT)
    Q_PROPERTY(int maxIndent READ maxIndent CONSTANT)

public:
    using QObject::QObject;

    QStringList icons() const;
    QStringList flags() const;
    QStringList colors() const;
    QStringList kinds() const;
    QStringList mailKinds() const;
    int maxIndent() const { return 3; }
    Q_INVOKABLE QString iconLabel(const QString& icon) const;
    Q_INVOKABLE QString flagLabel(const QString& flag) const;
    Q_INVOKABLE QString kindIcon(const QString& kind) const;
    Q_INVOKABLE QString kindLabel(const QString& kind) const;
    Q_INVOKABLE QStringList moves(const QString& kind) const;
};
