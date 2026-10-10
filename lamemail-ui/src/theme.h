#pragma once

#include <QColor>
#include <QHash>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QtQml/qqmlregistration.h>
#include <optional>

using Toml = QHash<QString, QString>;

Toml parseToml(const QString& text);
std::optional<QColor> hexColor(const QString& text);
QColor pickColor(const Toml& table, const QStringList& keys, const QString& fallback);
QString omarchyCurrent();

class Theme : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QColor background READ background NOTIFY changed)
    Q_PROPERTY(QColor backgroundDeep READ backgroundDeep NOTIFY changed)
    Q_PROPERTY(QColor surface READ surface NOTIFY changed)
    Q_PROPERTY(QColor foreground READ foreground NOTIFY changed)
    Q_PROPERTY(QColor foregroundBright READ foregroundBright NOTIFY changed)
    Q_PROPERTY(QColor muted READ muted NOTIFY changed)
    Q_PROPERTY(QColor line READ line NOTIFY changed)
    Q_PROPERTY(QColor accent READ accent NOTIFY changed)
    Q_PROPERTY(QColor red READ red NOTIFY changed)
    Q_PROPERTY(QColor yellow READ yellow NOTIFY changed)
    Q_PROPERTY(QColor orange READ orange NOTIFY changed)
    Q_PROPERTY(QColor green READ green NOTIFY changed)
    Q_PROPERTY(QColor cyan READ cyan NOTIFY changed)
    Q_PROPERTY(QColor blue READ blue NOTIFY changed)
    Q_PROPERTY(QColor magenta READ magenta NOTIFY changed)
    Q_PROPERTY(QString fontFamily READ fontFamily NOTIFY changed)
    Q_PROPERTY(int fontSize READ fontSize NOTIFY changed)
    Q_PROPERTY(int radius READ radius CONSTANT)

public:
    explicit Theme(QObject* parent = nullptr);

    Q_INVOKABLE void reload();

    QColor background() const { return m_palette.background; }
    QColor backgroundDeep() const { return m_palette.backgroundDeep; }
    QColor surface() const { return m_palette.surface; }
    QColor foreground() const { return m_palette.foreground; }
    QColor foregroundBright() const { return m_palette.foregroundBright; }
    QColor muted() const { return m_palette.muted; }
    QColor line() const { return m_palette.line; }
    QColor accent() const { return m_palette.accent; }
    QColor red() const { return m_palette.red; }
    QColor yellow() const { return m_palette.yellow; }
    QColor orange() const { return m_palette.orange; }
    QColor green() const { return m_palette.green; }
    QColor cyan() const { return m_palette.cyan; }
    QColor blue() const { return m_palette.blue; }
    QColor magenta() const { return m_palette.magenta; }
    QString fontFamily() const { return m_palette.fontFamily; }
    int fontSize() const { return m_palette.fontSize; }
    int radius() const { return 6; }

signals:
    void changed();

private:
    struct Palette {
        QColor background;
        QColor backgroundDeep;
        QColor surface;
        QColor foreground;
        QColor foregroundBright;
        QColor muted;
        QColor line;
        QColor accent;
        QColor red;
        QColor yellow;
        QColor orange;
        QColor green;
        QColor cyan;
        QColor blue;
        QColor magenta;
        QString fontFamily;
        int fontSize = 12;
    };

    static Palette load();

    Palette m_palette;
};
