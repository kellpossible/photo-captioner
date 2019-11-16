use std::error::Error;
use std::path::{PathBuf, Path};
use std::{fs, env, io};
use structopt::StructOpt;
use csv;
use cursive::Cursive;
use cursive::views::{Dialog, SelectView, EditView, ViewRef};
use cursive::traits::{Identifiable, Boxable};
use std::cell::{RefCell, Ref, RefMut};
use std::borrow::{Borrow, BorrowMut};
use std::rc::Rc;
use std::ops::Deref;


#[derive(Debug, StructOpt)]
#[structopt(name = "image-captioner", about = "Edit captions for a gallery of images.")]
struct Opt {
    /// Directory of the gallery to generate captions for
    #[structopt(parse(from_os_str))]
    gallery_dir: Option<PathBuf>,

    /// The type of output, available options: "csv"
    #[structopt(short = "t", long = "output-type", default_value = "csv")]
    output_type: String,

    /// The name of the output file (if there is one).
    /// Will be "captions.csv" by default for the "csv" output-type.
    #[structopt(short = "n", long = "output-name")]
    output_name: Option<String>,

    /// whether or not to edit the captions
    #[structopt(short, long)]
    edit: bool,
}

#[derive(Debug, Clone)]
struct CaptionRecord {
    pub image_path: PathBuf,
    pub caption: String,
}

impl CaptionRecord {
    fn new(image_path: &PathBuf, caption: String) -> CaptionRecord {
        CaptionRecord {
            image_path: image_path.clone(),
            caption: caption.clone(),
        }
    }

    fn empty_caption(image_path: &PathBuf) -> CaptionRecord {
        CaptionRecord::new(image_path, String::new())
    }

    fn get_filename(&self) -> &str {
        self.image_path.file_name().unwrap().to_str().unwrap()
    }

    fn get_label(&self) -> String {
        format!("{}: {}", self.get_filename(), self.caption)
    }
}

fn get_image_files(gallery_dir: &PathBuf) -> io::Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let supported_extensions = vec!["jpg", "jpeg", "png"];

    for entry in fs::read_dir(gallery_dir)? {
        let entry_path = entry?.path().clone();

        match entry_path.extension() {
            Some(ext) => {
                let ext_string = ext.to_str().expect("unable to convert path").to_lowercase();
                let ext_str = ext_string.as_str();

                if supported_extensions.contains(&ext_str)
                {
                    paths.push(entry_path);
                }
            },
            None => ()
        }
    }

    Ok(paths)
}

fn generate_empty_captions(image_paths: &Vec<PathBuf>) -> Vec<CaptionRecord> {
    let mut records: Vec<CaptionRecord> = Vec::new();

    for image_path in image_paths {
        records.push(CaptionRecord::empty_caption(&image_path))
    }

    return records;
}

fn read_caption_csv(csv_path: &Path) -> Result<Vec<CaptionRecord>, Box<dyn Error>> {
    let image_directory = csv_path.parent().expect("csv path is not a valid file").to_path_buf();
    let mut captions: Vec<CaptionRecord> = Vec::new();
    let mut rdr = csv::Reader::from_path(csv_path)?;

    for item in rdr.records() {
        let record = item?;
        let image_filename = record.get(0).expect("badly formatted image filename in csv");
        let caption = record.get(1).expect("badly formatted caption entry in csv");

        let image_path = image_directory.join(image_filename);

        let caption_record = CaptionRecord::new(&image_path, caption.to_owned());

        captions.push(caption_record);
    }

    return Ok(captions);
}

fn write_caption_csv(records: &Vec<CaptionRecord>, csv_path: &Path) -> Result<(), Box<dyn Error>> {
    println!("Writing captions to \"{}\".", csv_path.display());

    let mut wtr = csv::Writer::from_path(csv_path)?;
    wtr.write_record(&["Image", "Caption"])?;

    for record in records {
        let image_filename: &str = record.image_path.file_name().expect("expected image to be a filename").to_str().unwrap();
        wtr.write_record(&[image_filename, record.caption.as_str()])?;
    }

    Ok(())
}

fn edit_caption(s: &mut Cursive, record: Rc<RefCell<CaptionRecord>>) {
    let record_ref = RefCell::borrow(record.borrow());
    let caption_text = record_ref.caption.clone();
    let image_file_name = String::from(record_ref.get_filename().clone());

    let mut ev = EditView::new();
    ev.set_content(caption_text);
    s.add_layer(Dialog::around(ev.with_id("edit_caption")
            .fixed_width(10))
        .title(format!("Editing caption for image {}", image_file_name))
        .button("Ok", |s| {
            let new_caption_text: Rc<String> = s.call_on_id("edit_caption", |view: &mut EditView| {
                view.get_content()
            }).unwrap().clone();

            let mut select_view_ref: ViewRef<SelectView<Rc<RefCell<CaptionRecord>>>> = s.find_id::<SelectView<Rc<RefCell<CaptionRecord>>>>("select_image").unwrap();

            let selection = Rc::clone(select_view_ref.selection().unwrap().as_ref());
            let mut record_ref: RefMut<CaptionRecord> = RefCell::borrow_mut(Rc::borrow(&selection));
            record_ref.caption = new_caption_text.as_ref().clone();

            let selected_id = select_view_ref.selected_id().unwrap();
            let mut select_view_ref_mut = select_view_ref.borrow_mut();
            select_view_ref_mut.remove_item(selected_id);
            select_view_ref_mut.insert_item(selected_id, record_ref.get_label(), selection.clone());

            s.pop_layer();
            s.refresh();
        })
        .button("Cancel", |s| {
            s.pop_layer();
        }));
}

fn edit_captions(opt: &Opt, captions: &Vec<CaptionRecord>) -> Vec<CaptionRecord> {
    if !opt.edit {
        return captions.clone();
    }

    let mut editable_captions: Vec<Rc<RefCell<CaptionRecord>>> = Vec::new();

    for record in captions {
        editable_captions.push(Rc::new(RefCell::new(record.clone())))
    }

    // Creates the cursive root - required for every application.
    let mut siv = Cursive::default();

    let mut select_view = SelectView::<Rc<RefCell<CaptionRecord>>>::new();

    for record in editable_captions {
        let record_reference = RefCell::borrow(record.borrow());
        let image_file_name = String::from(record_reference.get_filename().clone());
        let caption = String::from(record_reference.caption.clone());
        let label = record_reference.get_label();
        select_view.add_item(label, record.clone());
    }

    select_view.set_on_submit(|s, record: &Rc<RefCell<CaptionRecord>>| {
        edit_caption(s, record.clone());
    });

    // Creates a dialog with a single "Quit" button
    siv.add_layer(Dialog::around(select_view.with_id("select_image"))
        .title("Caption Editor")
        .button("Quit", |s| s.quit()));

    // Starts the event loop.
    siv.run();

    return Vec::new();
}

fn main() {
    let opt = Opt::from_args();

    let gallery_dir = match opt.gallery_dir.clone() {
        Some(path) => path,
        None => env::current_dir().expect("Error: cannot get current directory")
    };

    let image_paths = get_image_files(&gallery_dir).expect("Error: unable to read image files from gallery directory");

    let output_type: String = opt.output_type.clone();
    match output_type.as_str() {
       "csv" => {
           let csv_filename: String = match opt.output_name.clone() {
               Some(name) => name,
               None => String::from("captions.csv")
           };

           let csv_path = gallery_dir.join(Path::new(csv_filename.as_str()));

           let mut captions = if csv_path.exists()
           {
               println!("Caption file \"{}\" already exists, reading file.", csv_filename);
               let mut captions = read_caption_csv(csv_path.as_path()).expect("unable to read captions csv");
               let mut images_with_no_cations: Vec<PathBuf> = Vec::new();

               for image_path in image_paths {
                   match captions.iter().find(|&record| {
                       record.image_path.canonicalize().unwrap().eq(&image_path.canonicalize().unwrap())
                   }) {
                       Some(_record) => (),
                       None => images_with_no_cations.push(image_path)
                   }
               }

               let mut new_captions = generate_empty_captions(&images_with_no_cations);

               println!("Appending the following new images: [{}]", new_captions.iter()
                   .fold(String::new(), |acc, record| {
                       acc + &record.image_path.file_name().unwrap().to_str().unwrap() + ", "
                   }));

               captions.append(&mut new_captions);

               captions.sort_by(|a, b| {
                   a.image_path.file_name().unwrap().cmp(b.image_path.file_name().unwrap())
               });

               captions
           } else {
               println!("Generating new captions.");
               generate_empty_captions(&image_paths)
           };

           edit_captions(&opt, &mut captions);

           write_caption_csv(&captions, csv_path.as_path()).expect("unable to write captions to csv");
       },
        _ => println!("Error: unsupported output type {}", output_type)
    }
}


