using System;
using System.Globalization;
using System.Windows;
using System.Windows.Data;
using PigTree.ViewModel;

namespace PigTree.Converters;

public sealed class BooleanToVisibilityConverter : IValueConverter
{
    public bool Inverse { get; set; }
    public Visibility FalseVisibility { get; set; } = Visibility.Collapsed;

    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        bool flag = value is true;
        if (Inverse) flag = !flag;
        return flag ? Visibility.Visible : FalseVisibility;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is Visibility vis)
        {
            bool flag = vis == Visibility.Visible;
            return Inverse ? !flag : flag;
        }
        return false;
    }
}

public sealed class LevelToIndentMarginConverter : IValueConverter
{
    public double Step { get; set; } = 16.0;

    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is int level)
        {
            return new Thickness(level * Step, 0, 0, 0);
        }
        if (value is uint uLevel)
        {
            return new Thickness(uLevel * Step, 0, 0, 0);
        }
        if (value is double dMargin)
        {
            return new Thickness(dMargin, 0, 0, 0);
        }
        if (value is TreeItemViewModel item)
        {
            return new Thickness(item.Level * Step, 0, 0, 0);
        }
        if (value is IConvertible convertible)
        {
            try
            {
                double val = System.Convert.ToDouble(convertible, culture);
                return new Thickness(val * Step, 0, 0, 0);
            }
            catch
            {
                // Fallback
            }
        }
        return new Thickness(0);
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => 0;
}

public sealed class ExpandCollapseIconConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is true)
        {
            return "▼";
        }
        return "▶";
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => false;
}

/// <summary>
/// Converts a boolean to a <see cref="GridViewColumn"/> width: columns cannot be
/// collapsed via Visibility, so hidden columns collapse to zero width instead.
/// </summary>
public sealed class BooleanToColumnWidthConverter : IValueConverter
{
    public double VisibleWidth { get; set; } = 130.0;

    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        return value is true ? VisibleWidth : 0.0;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
        => Binding.DoNothing;
}

/// <summary>
/// A Freezable proxy enabling bindings from visual/logical tree orphans (such as GridViewColumn)
/// to the ambient DataContext.
/// </summary>
public class BindingProxy : Freezable
{
    protected override Freezable CreateInstanceCore()
    {
        return new BindingProxy();
    }

    public object? Data
    {
        get => GetValue(DataProperty);
        set => SetValue(DataProperty, value);
    }

    public static readonly DependencyProperty DataProperty =
        DependencyProperty.Register(nameof(Data), typeof(object), typeof(BindingProxy), new UIPropertyMetadata(null));
}
