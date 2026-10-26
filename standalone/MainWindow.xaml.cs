using System.Windows;

namespace CopyAll
{
    public partial class MainWindow : Window
    {
        public MainWindow()
        {
            InitializeComponent();
        }

        private void BtnInstall_Click(object sender, RoutedEventArgs e)
        {
            App.InstallContextMenu();
            Close();
        }

        private void BtnRemove_Click(object sender, RoutedEventArgs e)
        {
            App.RemoveContextMenu();
            Close();
        }

        private void BtnClose_Click(object sender, RoutedEventArgs e)
        {
            Close();
        }
    }
}